#!/usr/bin/env python3
"""Convert a MoveNet TFLite model into the pair of files this example loads.

    pip install tflite numpy
    python3 convert_movenet.py model/movenet_lightning_f16.tflite -o model/

Writes `movenet.json` (the layer list) and `movenet.safetensors` (the weights,
already transposed into the layout ggml wants).

Why a converter instead of hard-coded layers: the example builds whatever
topology this script reports, so Lightning, Thunder and retrained variants all
run without touching Rust.

The published models are `center_net_mobile_net_v2fpn_feature_extractor`
graphs that

  * take uint8 input and normalize it with a CAST/SUB/MUL/SUB chain,
  * keep float16 weights behind DEQUANTIZE ops,
  * end with argmax/gather post-processing that turns the four prediction
    heads into a [1, 1, 17, 3] keypoint list.

This script folds the dequantize ops away, keeps the normalization as explicit
layers, and stops at the four heads — the Rust side decodes them.

Models:
    https://tfhub.dev/google/lite-model/movenet/singlepose/lightning/tflite/float16/4?lite-format=tflite
    https://tfhub.dev/google/lite-model/movenet/singlepose/thunder/tflite/float16/4?lite-format=tflite
"""

import argparse
import json
import struct
import sys
from pathlib import Path

import numpy as np

try:
    import tflite
except ImportError:  # pragma: no cover - user environment
    sys.exit("this converter needs the flatbuffer schema package: pip install tflite")

from tflite.ActivationFunctionType import ActivationFunctionType
from tflite.BuiltinOperator import BuiltinOperator
from tflite.Conv2DOptions import Conv2DOptions
from tflite.DepthwiseConv2DOptions import DepthwiseConv2DOptions
from tflite.ConcatenationOptions import ConcatenationOptions
from tflite.Padding import Padding

HEAD_CHANNELS = {"heatmap": 17, "center": 1, "regress": 34, "offset": 34}
# the 34-channel heads are told apart by these substrings in their tensor names
HEAD_HINTS = {"regress": "regress", "offset": "offset"}

ACTIVATIONS = {
    ActivationFunctionType.NONE: "none",
    ActivationFunctionType.RELU: "relu",
    ActivationFunctionType.RELU6: "relu6",
    ActivationFunctionType.TANH: "tanh",
}

TFLITE_DTYPES = {0: np.float32, 1: np.float16, 2: np.int32, 3: np.uint8, 9: np.int8}


class Converter:
    def __init__(self, model, subgraph):
        self.model = model
        self.sg = subgraph
        self.layers = []
        self.tensors = {}      # weight name -> np.ndarray
        self.alias = {}        # tensor index -> tensor index it passes through to
        self.constants = {}    # tensor index -> np.ndarray, folded dequantized weights
        self.emitted = {}      # tensor index -> layer output name
        self.warnings = []
        self.asymmetric = 0    # layers needing explicit padding

    # -- tensor helpers ---------------------------------------------------
    def resolve(self, index):
        seen = set()
        while index in self.alias and index not in seen:
            seen.add(index)
            index = self.alias[index]
        return index

    def raw_name(self, index):
        n = self.sg.Tensors(index).Name()
        return n.decode("utf-8") if n is not None else f"t{index}"

    def name(self, index):
        index = self.resolve(index)
        # tflite names are long and contain '/' and ';'; keep them unique but tidy
        return self.raw_name(index).replace(" ", "_")

    def shape(self, index):
        t = self.sg.Tensors(self.resolve(index))
        return [t.Shape(i) for i in range(t.ShapeLength())]

    def constant(self, index):
        """Constant value of a tensor, following dequantize aliases."""
        index = self.resolve(index)
        if index in self.constants:
            return self.constants[index]
        t = self.sg.Tensors(index)
        buf = self.model.Buffers(t.Buffer())
        if buf.DataLength() == 0:
            return None
        dtype = TFLITE_DTYPES.get(t.Type())
        if dtype is None:
            raise SystemExit(
                f"tensor {self.raw_name(index)} has dtype code {t.Type()}; "
                "use the float16 or float32 model, not the int8 one"
            )
        raw = buf.DataAsNumpy().tobytes()
        return np.frombuffer(raw, dtype=dtype).reshape(self.shape(index)).astype(np.float32)

    def store(self, name, array):
        self.tensors[name] = np.ascontiguousarray(array, dtype=np.float32)
        return name

    # -- padding ----------------------------------------------------------
    def explicit_pad(self, in_size, k, stride, dilation, padding):
        """TFLite records SAME/VALID; return (before, after) pixel counts.

        SAME puts the odd pixel on the right/bottom, so the two sides differ
        whenever the total is odd. The Rust side pads explicitly in that case.
        """
        if padding == Padding.VALID:
            return 0, 0
        effective = (k - 1) * dilation + 1
        out = (in_size + stride - 1) // stride
        total = max((out - 1) * stride + effective - in_size, 0)
        return total // 2, total - total // 2

    # -- op handlers ------------------------------------------------------
    def op_conv(self, op, depthwise):
        inputs = [op.Inputs(i) for i in range(op.InputsLength())]
        src, weight_idx = inputs[0], inputs[1]
        bias_idx = inputs[2] if len(inputs) > 2 else -1
        out_idx = op.Outputs(0)
        out_name = self.name(out_idx)

        opts = DepthwiseConv2DOptions() if depthwise else Conv2DOptions()
        opts.Init(op.BuiltinOptions().Bytes, op.BuiltinOptions().Pos)

        stride = [int(opts.StrideW()), int(opts.StrideH())]
        dilation = [int(opts.DilationWFactor()), int(opts.DilationHFactor())]
        activation = ACTIVATIONS.get(opts.FusedActivationFunction())
        if activation is None:
            raise SystemExit(f"{out_name}: unsupported fused activation")

        w = self.constant(weight_idx)
        if w is None:
            raise SystemExit(f"{out_name}: convolution weights are not constant")

        in_shape = self.shape(src)  # NHWC
        if depthwise:
            if opts.DepthMultiplier() != 1:
                raise SystemExit(f"{out_name}: depth_multiplier != 1 is not supported")
            # [1, KH, KW, C] -> ggml [C, 1, KH, KW]
            _, kh, kw, _ = w.shape
            w = np.transpose(w, (3, 0, 1, 2))
        else:
            # [OC, KH, KW, IC] -> ggml [OC, IC, KH, KW]
            _, kh, kw, _ = w.shape
            w = np.transpose(w, (0, 3, 1, 2))

        left, right = self.explicit_pad(in_shape[2], kw, stride[0], dilation[0], opts.Padding())
        top, bottom = self.explicit_pad(in_shape[1], kh, stride[1], dilation[1], opts.Padding())

        layer = {
            "op": "dwconv2d" if depthwise else "conv2d",
            "inputs": [self.name(src)],
            "output": out_name,
            "weight": self.store(f"{out_name}.weight", w),
            "stride": stride,
            "pad": [left, top],
            "dilation": dilation,
            "activation": activation,
        }
        if left != right or top != bottom:
            layer["pad_lrtb"] = [left, right, top, bottom]
            self.asymmetric += 1

        if bias_idx >= 0:
            b = self.constant(bias_idx)
            if b is not None:
                layer["bias"] = self.store(f"{out_name}.bias", b.reshape(-1))

        self.emit(layer, out_idx)

    def op_elementwise(self, op, kind):
        """ADD/SUB/MUL/DIV, with constant operands materialized as literals."""
        out_idx = op.Outputs(0)
        out_name = self.name(out_idx)
        inputs = []
        for i in range(op.InputsLength()):
            idx = op.Inputs(i)
            value = self.constant(idx)
            if value is None:
                inputs.append(self.name(idx))
                continue
            # a literal operand: emit it as its own layer so the graph builder
            # can upload and broadcast it
            const_name = f"{out_name}.const{i}"
            self.layers.append(
                {
                    "op": "constant",
                    "inputs": [],
                    "output": const_name,
                    "weight": self.store(const_name, value),
                }
            )
            inputs.append(const_name)

        self.emit({"op": kind, "inputs": inputs, "output": out_name}, out_idx)

    def op_activation(self, op, activation):
        out_idx = op.Outputs(0)
        self.emit(
            {
                "op": "activation",
                "inputs": [self.name(op.Inputs(0))],
                "output": self.name(out_idx),
                "activation": activation,
            },
            out_idx,
        )

    def op_resize(self, op, mode):
        out_idx = op.Outputs(0)
        src = op.Inputs(0)
        in_shape, out_shape = self.shape(src), self.shape(out_idx)
        if in_shape[1] == 0 or out_shape[1] % in_shape[1] != 0:
            raise SystemExit(
                f"{self.name(out_idx)}: resize {in_shape} -> {out_shape} is not an integer "
                "upscale, which is all the ggml upscale op supports"
            )
        self.emit(
            {
                "op": "upsample",
                "inputs": [self.name(src)],
                "output": self.name(out_idx),
                "factor": int(out_shape[1] // in_shape[1]),
                "mode": mode,
            },
            out_idx,
        )

    def op_concat(self, op):
        opts = ConcatenationOptions()
        opts.Init(op.BuiltinOptions().Bytes, op.BuiltinOptions().Pos)
        out_idx = op.Outputs(0)
        rank = len(self.shape(out_idx))
        axis = opts.Axis()
        if axis < 0:
            axis += rank
        # NHWC axis -> ggml dim, given ggml keeps ne[0] fastest:
        #   rank 4 [N,H,W,C] -> [W,H,C,N];  rank 3 [N,L,C] -> [C,L,1,N]
        table = {4: {1: 1, 2: 0, 3: 2}, 3: {1: 1, 2: 0}, 2: {1: 0}}
        dim = table.get(rank, {}).get(axis)
        if dim is None:
            raise SystemExit(
                f"{self.name(out_idx)}: cannot concatenate rank-{rank} tensors on axis {axis}"
            )
        self.emit(
            {
                "op": "concat",
                "inputs": [self.name(op.Inputs(i)) for i in range(op.InputsLength())],
                "output": self.name(out_idx),
                "dim": dim,
            },
            out_idx,
        )

    def op_prelu(self, op):
        out_idx = op.Outputs(0)
        slope = self.constant(op.Inputs(1))
        if slope is None:
            raise SystemExit(f"{self.name(out_idx)}: prelu slope is not constant")
        name = self.name(out_idx)
        self.emit(
            {
                "op": "prelu",
                "inputs": [self.name(op.Inputs(0))],
                "output": name,
                "slope": self.store(f"{name}.slope", np.ravel(slope)),
            },
            out_idx,
        )

    def op_pool(self, op, kind):
        from tflite.Pool2DOptions import Pool2DOptions
        opts = Pool2DOptions()
        opts.Init(op.BuiltinOptions().Bytes, op.BuiltinOptions().Pos)
        out_idx = op.Outputs(0)
        src = op.Inputs(0)
        in_shape = self.shape(src)
        stride = [int(opts.StrideW()), int(opts.StrideH())]
        kw, kh = int(opts.FilterWidth()), int(opts.FilterHeight())
        left, right = self.explicit_pad(in_shape[2], kw, stride[0], 1, opts.Padding())
        top, bottom = self.explicit_pad(in_shape[1], kh, stride[1], 1, opts.Padding())
        layer = {
            "op": kind,
            "inputs": [self.name(src)],
            "output": self.name(out_idx),
            "window": [kw, kh],
            "stride": stride,
            "pad": [left, top],
            "activation": ACTIVATIONS.get(opts.FusedActivationFunction(), "none"),
        }
        if left != right or top != bottom:
            layer["pad_lrtb"] = [left, right, top, bottom]
            self.asymmetric += 1
        self.emit(layer, out_idx)

    def op_pad(self, op):
        out_idx = op.Outputs(0)
        pads = self.constant(op.Inputs(1))
        if pads is None:
            raise SystemExit(f"{self.name(out_idx)}: pad amounts are not constant")
        pads = pads.astype(int)  # [[n0,n1],[h0,h1],[w0,w1],[c0,c1]]
        if pads.shape[0] != 4 or pads[0].any():
            raise SystemExit(f"{self.name(out_idx)}: batch padding is not supported")
        layer = {
            "op": "pad",
            "inputs": [self.name(op.Inputs(0))],
            "output": self.name(out_idx),
            "pad_lrtb": [int(pads[2][0]), int(pads[2][1]), int(pads[1][0]), int(pads[1][1])],
        }
        if pads[3].any():
            # channel padding, used by the palm detector's shortcut branches
            layer["pad_channels"] = [int(pads[3][0]), int(pads[3][1])]
        self.emit(layer, out_idx)

    def op_reshape(self, op):
        out_idx = op.Outputs(0)
        self.emit(
            {
                "op": "reshape",
                "inputs": [self.name(op.Inputs(0))],
                "output": self.name(out_idx),
                "shape": [int(v) for v in self.shape(out_idx)],
            },
            out_idx,
        )

    def op_fully_connected(self, op):
        out_idx = op.Outputs(0)
        name = self.name(out_idx)
        w = self.constant(op.Inputs(1))
        if w is None:
            raise SystemExit(f"{name}: fully_connected weights are not constant")
        layer = {
            "op": "fully_connected",
            "inputs": [self.name(op.Inputs(0))],
            "output": name,
            "weight": self.store(f"{name}.weight", w),
        }
        if op.InputsLength() > 2 and op.Inputs(2) >= 0:
            b = self.constant(op.Inputs(2))
            if b is not None:
                layer["bias"] = self.store(f"{name}.bias", np.ravel(b))
        self.emit(layer, out_idx)

    def op_mean(self, op):
        out_idx = op.Outputs(0)
        axes = self.constant(op.Inputs(1))
        if axes is None or sorted(int(a) for a in np.ravel(axes)) != [1, 2]:
            raise SystemExit(f"{self.name(out_idx)}: only spatial mean (axes 1,2) is supported")
        self.emit(
            {"op": "mean", "inputs": [self.name(op.Inputs(0))], "output": self.name(out_idx)},
            out_idx,
        )

    def emit(self, layer, out_idx):
        self.layers.append(layer)
        self.emitted[self.resolve(out_idx)] = layer["output"]

    # -- driver -----------------------------------------------------------
    def required_ops(self, head_indices):
        """Ops the heads actually depend on, found by walking backwards.

        The published graphs interleave post-processing (reshape, argmax,
        gather) with the layers we want, so position in the op list is not a
        reliable cut point."""
        producer = {}
        for i in range(self.sg.OperatorsLength()):
            op = self.sg.Operators(i)
            for j in range(op.OutputsLength()):
                producer[op.Outputs(j)] = i

        needed, stack = set(), list(head_indices)
        while stack:
            tensor = stack.pop()
            i = producer.get(tensor)
            if i is None or i in needed:
                continue
            needed.add(i)
            op = self.sg.Operators(i)
            for j in range(op.InputsLength()):
                src = op.Inputs(j)
                if src >= 0:
                    stack.append(src)
        return needed

    def run(self, head_indices):
        """Convert every op the heads depend on, in graph order."""
        opcodes = [self.model.OperatorCodes(i) for i in range(self.model.OperatorCodesLength())]
        needed = self.required_ops(head_indices)
        pending = set(head_indices)

        for i in sorted(needed):
            op = self.sg.Operators(i)
            code = opcodes[op.OpcodeIndex()]
            builtin = max(code.BuiltinCode(), code.DeprecatedBuiltinCode())
            out_idx = op.Outputs(0)

            if builtin == BuiltinOperator.DEQUANTIZE:
                # float16 weights unpacked at runtime: fold to a constant, or
                # pass an activation straight through
                value = self.constant(op.Inputs(0))
                if value is not None:
                    self.constants[self.resolve(out_idx)] = value
                else:
                    self.alias[out_idx] = op.Inputs(0)
            elif builtin == BuiltinOperator.CAST:
                # uint8 -> float on the model input
                self.alias[out_idx] = op.Inputs(0)
            elif builtin == BuiltinOperator.RESHAPE and self.constant(op.Inputs(0)) is not None:
                self.constants[self.resolve(out_idx)] = self.constant(op.Inputs(0)).reshape(
                    self.shape(out_idx)
                )
            elif builtin == BuiltinOperator.CONV_2D:
                self.op_conv(op, depthwise=False)
            elif builtin == BuiltinOperator.DEPTHWISE_CONV_2D:
                self.op_conv(op, depthwise=True)
            elif builtin == BuiltinOperator.ADD:
                self.op_elementwise(op, "add")
            elif builtin == BuiltinOperator.SUB:
                self.op_elementwise(op, "sub")
            elif builtin == BuiltinOperator.MUL:
                self.op_elementwise(op, "mul")
            elif builtin == BuiltinOperator.DIV:
                self.op_elementwise(op, "div")
            elif builtin == BuiltinOperator.CONCATENATION:
                self.op_concat(op)
            elif builtin == BuiltinOperator.LOGISTIC:
                self.op_activation(op, "sigmoid")
            elif builtin == BuiltinOperator.RELU:
                self.op_activation(op, "relu")
            elif builtin == BuiltinOperator.RELU6:
                self.op_activation(op, "relu6")
            elif builtin == BuiltinOperator.HARD_SWISH:
                self.op_activation(op, "hardswish")
            elif builtin == BuiltinOperator.RESIZE_BILINEAR:
                self.op_resize(op, "bilinear")
            elif builtin == BuiltinOperator.RESIZE_NEAREST_NEIGHBOR:
                self.op_resize(op, "nearest")
            elif builtin == BuiltinOperator.PRELU:
                self.op_prelu(op)
            elif builtin == BuiltinOperator.MAX_POOL_2D:
                self.op_pool(op, "maxpool2d")
            elif builtin == BuiltinOperator.AVERAGE_POOL_2D:
                self.op_pool(op, "avgpool2d")
            elif builtin == BuiltinOperator.PAD:
                self.op_pad(op)
            elif builtin == BuiltinOperator.RESHAPE:
                self.op_reshape(op)
            elif builtin == BuiltinOperator.FULLY_CONNECTED:
                self.op_fully_connected(op)
            elif builtin == BuiltinOperator.MEAN:
                self.op_mean(op)
            else:
                raise SystemExit(
                    f"operator #{i} ({self.raw_name(out_idx)}) has builtin code {builtin}, "
                    "which is not supported. The heads depend on it, so the graph cannot be "
                    "converted as-is."
                )

            pending.discard(self.resolve(out_idx))

        if pending:
            missing = ", ".join(self.raw_name(i) for i in pending)
            raise SystemExit(f"these heads were never produced: {missing}")


def find_heads(sg, model, overrides):
    """Locate the four heads by output channel count, disambiguated by name."""
    opcodes = [model.OperatorCodes(i) for i in range(model.OperatorCodesLength())]
    candidates = {}  # channels -> [(op index, tensor index, name)]
    for i in range(sg.OperatorsLength()):
        op = sg.Operators(i)
        code = opcodes[op.OpcodeIndex()]
        builtin = max(code.BuiltinCode(), code.DeprecatedBuiltinCode())
        if builtin not in (BuiltinOperator.CONV_2D, BuiltinOperator.LOGISTIC):
            continue
        idx = op.Outputs(0)
        t = sg.Tensors(idx)
        shape = [t.Shape(j) for j in range(t.ShapeLength())]
        if len(shape) != 4:
            continue
        raw = t.Name()
        candidates.setdefault(shape[3], []).append(
            (i, idx, raw.decode("utf-8") if raw else f"t{idx}")
        )

    heads = {}
    for role, channels in HEAD_CHANNELS.items():
        override = overrides.get(role)
        if override:
            match = [c for c in sum(candidates.values(), []) if c[2] == override]
            if not match:
                raise SystemExit(f"--{role}-tensor {override!r} matches no operator output")
            heads[role] = match[-1]
            continue

        pool = candidates.get(channels, [])
        if not pool:
            raise SystemExit(f"no operator produces {channels} channels; pass --{role}-tensor")
        hint = HEAD_HINTS.get(role)
        if hint:
            hinted = [c for c in pool if hint in c[2]]
            if not hinted:
                raise SystemExit(
                    f"no {channels}-channel tensor mentions {hint!r}; pass --{role}-tensor"
                )
            pool = hinted
        # the sigmoid, not the conv feeding it, is the head we want
        heads[role] = pool[-1]

    if heads["regress"][1] == heads["offset"][1]:
        raise SystemExit("regress and offset resolved to the same tensor; pass them explicitly")
    return heads


def write_safetensors(path, tensors):
    header, offset = {}, 0
    for name, array in tensors.items():
        header[name] = {
            "dtype": "F32",
            "shape": list(array.shape),
            "data_offsets": [offset, offset + array.nbytes],
        }
        offset += array.nbytes
    blob = json.dumps(header, separators=(",", ":")).encode("utf-8")
    with open(path, "wb") as f:
        f.write(struct.pack("<Q", len(blob)))
        f.write(blob)
        for array in tensors.values():
            f.write(array.tobytes())


def main():
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("model", help="MoveNet .tflite file (float16 or float32)")
    ap.add_argument("-o", "--out", default="model", help="output directory")
    ap.add_argument("--name", default=None, help="model name recorded in the spec")
    ap.add_argument("--movenet", action="store_true",
                    help="cut the graph at MoveNet's four prediction heads instead of using "
                         "the model's declared outputs")
    ap.add_argument("--output", action="append", default=[], metavar="ROLE=TENSOR",
                    help="name an output explicitly; repeatable. Without any, the model's "
                         "own outputs are used, named out0, out1, ...")
    for role in HEAD_CHANNELS:
        ap.add_argument(f"--{role}-tensor", default=None, help=f"tensor name of the {role} head")
    args = ap.parse_args()

    buf = Path(args.model).read_bytes()
    model = tflite.Model.GetRootAsModel(buf, 0)
    if model.SubgraphsLength() != 1:
        raise SystemExit("expected a single-subgraph model")
    sg = model.Subgraphs(0)

    conv = Converter(model, sg)
    if args.movenet:
        overrides = {role: getattr(args, f"{role}_tensor") for role in HEAD_CHANNELS}
        heads = find_heads(sg, model, overrides)
        wanted = {role: idx for role, (_, idx, _) in heads.items()}
    elif args.output:
        by_name = {}
        for i in range(sg.TensorsLength()):
            n = sg.Tensors(i).Name()
            if n is not None:
                by_name[n.decode("utf-8")] = i
        wanted = {}
        for pair_str in args.output:
            role, _, tensor = pair_str.partition("=")
            if tensor not in by_name:
                raise SystemExit(f"--output {pair_str}: no tensor named {tensor}")
            wanted[role] = by_name[tensor]
    else:
        wanted = {f"out{i}": sg.Outputs(i) for i in range(sg.OutputsLength())}

    conv.run(set(wanted.values()))

    input_idx = sg.Inputs(0)
    in_shape = conv.shape(input_idx)
    if len(in_shape) != 4 or in_shape[3] != 3:
        raise SystemExit(f"unexpected model input shape {in_shape}, wanted [1, H, W, 3]")

    # the graph input feeds the normalization chain under its own name
    source = conv.name(input_idx)
    for layer in conv.layers:
        layer["inputs"] = ["image" if n == source else n for n in layer["inputs"]]

    spec = {
        "name": args.name or Path(args.model).stem,
        "input_width": int(in_shape[2]),
        "input_height": int(in_shape[1]),
        # MoveNet carries its own uint8 normalization chain, so it wants raw
        # 0..255; MediaPipe graphs take 0..1 floats
        "input_scale": 255.0 if args.movenet else 1.0,
        "input_bias": 0.0,
        "layers": conv.layers,
        "outputs": {role: conv.emitted[conv.resolve(idx)] for role, idx in wanted.items()},
    }

    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "movenet.json").write_text(json.dumps(spec, indent=1))
    write_safetensors(out_dir / "movenet.safetensors", conv.tensors)

    total = sum(a.nbytes for a in conv.tensors.values())
    print(f"wrote {out_dir/'movenet.json'} ({len(conv.layers)} layers)")
    print(f"wrote {out_dir/'movenet.safetensors'} ({len(conv.tensors)} tensors, {total/1e6:.1f} MB)")
    print(f"  input {in_shape[2]}x{in_shape[1]}")
    for role, idx in wanted.items():
        print(f"  {role:10s} <- {conv.emitted[conv.resolve(idx)].split(';')[0][-55:]} "
              f"{conv.shape(idx)}")
    if conv.asymmetric:
        print(f"  {conv.asymmetric} layers use asymmetric SAME padding, emitted as explicit pads")
    for w in dict.fromkeys(conv.warnings):
        print(f"  warning: {w}")


if __name__ == "__main__":
    main()
