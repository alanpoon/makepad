# Hand landmarks (MediaPipe)

Runs both stages of MediaPipe's Hand Landmarker on `makepad-ggml` via Metal:
palm detection finds each hand, then the landmark model runs on a rotated crop
of it. Draws the 21 landmarks with one color per finger.

```
cargo run -p makepad-example-hand-landmarks -- \
  examples/hand_landmarks/assets/hand.jpg examples/hand_landmarks/model
```

`assets/hand.jpg` is MediaPipe's own `victory.jpg` test image (from
`storage.googleapis.com/mediapipe-assets`, Apache-2.0 like the rest of
MediaPipe). On it the model reports presence 0.99, right hand 100%, and the
landmarks land on the fingers — a good baseline to check against after
changing anything.

Other MediaPipe test images in the same bucket work too: `thumb_up`,
`pointing_up`, `fist`, `left_hands`, `right_hands`.

## What works

* **Palm detection** — SSD over 2016 anchors, sigmoid scores, greedy NMS
* **Rotated ROI** — the crop is oriented so the hand points up, squared off on
  its long side, expanded 2.6x and shifted along the hand's own axis, matching
  MediaPipe's `DetectionsToRects` + `RectTransformation` settings
* 21 landmarks per hand, in MediaPipe's order (wrist, then thumb → pinky,
  knuckle → tip), mapped back through the rotation onto the source image
* Left/right classification with its confidence, and a hand-presence score
* World landmarks (the model's roughly-metric, hand-centered space)
* Multiple hands, up to `DetectParams::max_hands`
* ~135 ms per frame for both stages (detector 192x192, landmarks 224x224)

The ROI crop is what makes the landmark scores good: on the bundled photo the
cropped hand scores presence **0.99**, where feeding the whole frame to the
landmark model scores 0.385.

## What is missing — read this before trusting it

* **No video tracking.** MediaPipe reuses the previous frame's landmarks as
  the next frame's ROI and only re-runs detection when tracking is lost. Here
  every frame pays for both stages.
* **Plain NMS.** MediaPipe uses weighted NMS, which averages overlapping
  boxes; this keeps the highest-scoring one, so boxes shift slightly versus
  the reference.
* **Anchors are hard-coded** for the 192x192 two-grid layout (24x24x2 plus
  12x12x6 = 2016). A different anchor configuration needs `ANCHOR_GRIDS`
  updated; the count is checked against the model and fails loudly.
* **Not diffed against MediaPipe's own output.** The converted graph produces
  sensible landmarks on MediaPipe's test images (presence 0.96–0.999 across
  `thumb_up`, `pointing_up`, `victory`, `fist`, `left_hands`, `right_hands`,
  with handedness correct on the left/right pair), but nobody has run the same
  images through MediaPipe and compared coordinates the way the MoveNet
  example was compared against TFLite. Treat exact positions as unvalidated.

## How the stages fit together

| file | what it does |
| --- | --- |
| `src/palm.rs` | detector model, anchors, SSD decode, NMS |
| `src/roi.rs` | rotated crop and the mapping back out of it |
| `src/pipeline.rs` | detection → crop → landmarks for every hand |
| `src/hand.rs` | landmark model and its decode |
| `src/overlay.rs` | rasterizes the skeleton into the frame |

The detector needed one non-obvious thing in `libs/nn_graph`: its last ops
flatten `[1, H, W, C]` into `[1, N, 18]`, which is contiguous in TFLite's HWC
order but *not* in ggml's planar CHW. The `reshape` op now permutes spatial
tensors to channel-fastest before reinterpreting the shape — without that the
2016 anchors come out interleaved wrongly. PReLU, max pooling, and spatial and
channel padding were added for this model too.

## Getting the model

`model/` holds a converted copy plus the original bundle. To rebuild it:

```
curl -L -o hand_landmarker.task \
  "https://storage.googleapis.com/mediapipe-models/hand_landmarker/hand_landmarker/float16/1/hand_landmarker.task"
unzip hand_landmarker.task            # -> hand_landmarks_detector.tflite, hand_detector.tflite

pip install tflite numpy
CONV=../../libs/nn_graph/tools/convert_tflite.py

python3 $CONV hand_landmarks_detector.tflite -o model/
mv model/movenet.json model/hand_landmarks.json
mv model/movenet.safetensors model/hand_landmarks.safetensors

python3 $CONV hand_detector.tflite -o model/ --name palm_detector
mv model/movenet.json model/palm_detector.json
mv model/movenet.safetensors model/palm_detector.safetensors
```

The converter names outputs `out0..out3`; this example reads them as
landmarks, presence, handedness, world landmarks in that order.

## Tests

```
cargo test -p makepad-example-hand-landmarks
```

28 unit tests cover the anchor layout, SSD decoding against an anchor, score
thresholding, NMS (suppression, two separate hands, the hand limit, IoU
edges), ROI geometry (rotation for upright and sideways hands, the round trip
through `crop_to_source`, crop sampling and padding), landmark decoding, and
the overlay — plus an integration test that runs both stages on the bundled
photo and checks the palm score, the presence score, and that the landmarks
land on the picture.
