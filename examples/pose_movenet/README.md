# Human pose estimation with MoveNet

A makepad app that runs MoveNet single-pose over an image and draws the
17-keypoint COCO skeleton on top of it. Inference runs on `makepad-ggml`
through the Metal backend — no TensorFlow, no ONNX Runtime, no C++ at runtime.

```
cargo run -p makepad-example-pose-movenet -- photo.jpg model/
```

Both arguments are optional and default to `./pose.jpg` and `./model`. Without
a model the app still shows the picture and explains what is missing.

A test photo ships in `assets/pose.jpg` (1200x1465, the image the official
MoveNet tutorial uses, from Pexels under the Pexels license):

```
cargo run -p makepad-example-pose-movenet -- examples/pose_movenet/assets/pose.jpg model/
```

## Getting a model

The weights are not in the repo (`model/` is gitignored). Download a MoveNet
TFLite model — float16 or float32, **not** int8 — and convert it:

```
mkdir -p model
curl -L -o model/movenet_lightning_f16.tflite \
  "https://tfhub.dev/google/lite-model/movenet/singlepose/lightning/tflite/float16/4?lite-format=tflite"

pip install tflite numpy
python3 tools/convert_movenet.py model/movenet_lightning_f16.tflite -o model/
```

Thunder works the same way — swap `lightning` for `thunder` in the URL. It is
256x256 instead of 192x192 and about 2.6x the weights.

That writes two files the app loads:

* `model/movenet.json` — the layer list
* `model/movenet.safetensors` — the weights, transposed into ggml's layout

## How it fits together

| file | what it does |
| --- | --- |
| `src/movenet/spec.rs` | the model description the converter writes |
| `src/movenet/weights.rs` | minimal safetensors reader (F32/F16) |
| `src/movenet/graph.rs` | spec + weights → ggml graph |
| `src/movenet/preprocess.rs` | letterbox an image into the square model input |
| `src/movenet/decode.rs` | four heads → 17 keypoints |
| `src/overlay.rs` | rasterizes the skeleton into the frame |
| `src/app.rs` | the makepad UI |

**The topology is data, not code.** Rather than hard-coding an assumed
MobileNetV2 + FPN arrangement, the converter walks the real TFLite graph and
emits the layer list, so Lightning, Thunder, and retrained variants all work
without touching Rust. The op vocabulary is small: `conv2d`, `dwconv2d`, `add`,
`mul`, `concat`, `upsample`, `activation`.

Decoding follows the reference MoveNet post-processing: pick the person center
from the center heatmap (biased toward the middle of the frame), regress a
coarse position per joint from that cell, refine with a distance-weighted
argmax over each joint's heatmap channel, then apply the sub-pixel offset at
the winning cell.

## Layout conventions

ggml's `ne[0]` varies fastest, so activations are `[W, H, C, N]` (planar) and
convolution kernels are `[KW, KH, IC, OC]`, which is byte-identical to a
row-major `[OC, IC, KH, KW]` array. The converter transposes TFLite's
`[OC, KH, KW, IC]` accordingly, and depthwise `[1, KH, KW, C]` into
`[C, 1, KH, KW]`.

TensorFlow's `SAME` padding puts the odd pixel on the right/bottom, which
ggml's evenly-padding conv ops cannot express. The converter emits explicit
`pad_lrtb` for those layers (5 of them in Lightning) and the graph builder
inserts a `pad_ext` before the convolution, so both the output size and the
pixel grid match TensorFlow exactly.

## Known limitations

* **Metal only.** `makepad-ggml` implements Metal and CUDA; there is no CPU
  fallback and no Android/Vulkan backend, so this example is macOS-only today.
* **Single pose.** Only the single-pose models are handled; MultiPose
  Lightning outputs a different head layout.
* **No int8.** Quantized TFLite builds are rejected; use a float model.
* **Not compared against the TFLite reference.** The published Lightning
  weights load and produce an anatomically coherent skeleton on the bundled
  photo, but no one has run the same image through TensorFlow and diffed the
  coordinates. Until that happens, treat accuracy as unverified — the decode
  weighting constants in particular are a judgement call.
* **Decode constants.** `DecodeParams` uses inverse-distance weighting with a
  bias of 1.0 for both the center pick and the keypoint refinement. These
  affect which cell wins, not the coordinate read out of it; adjust them with
  `Estimator::set_decode_params` if your model was trained differently.

## Tests

```
cargo test -p makepad-example-pose-movenet
```

25 tests: safetensors parsing, spec parsing, letterbox math and its inverse,
head decoding (peak selection, center bias, regression targeting), skeleton
rasterization and clipping, an end-to-end build-and-run of a small synthetic
model on Metal, and — when `model/` holds a converted model — a full run of the
real network over `assets/pose.jpg`. Tests needing Metal or the downloaded
model skip themselves when those are unavailable.
