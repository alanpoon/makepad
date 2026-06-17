# SherpaAsrInput Widget — Design Spec

**Date:** 2026-06-17
**Status:** Approved
**Location:** `libs/asr_widget/` + updated `examples/speech_to_text/`

---

## 1. Purpose

Refactor the speech recognition widget from `examples/speech_to_text` into a reusable, standalone Makepad widget crate (`libs/asr_widget/`) backed by `sherpa-onnx` for on-device streaming ASR. The widget exposes a `model_dir` live field so any Makepad app can point at a pre-downloaded sherpa-onnx model directory and get real-time transcription with no cloud dependency.

---

## 2. Scope

**In scope:**
- New crate `libs/asr_widget/` with `SherpaAsrInput` widget
- Streaming recognition (interim results while speaking, final result on endpoint)
- `model_dir: String` live field — path to a sherpa-onnx model directory
- Auto-detection of Transducer (Zipformer/LSTM-RNN-T) and streaming CTC model layouts
- Amplitude visualization (mic button pulses while recording)
- Update `examples/speech_to_text/` to use `SherpaAsrInput`; remove `makepad-voice` dependency

**Out of scope:**
- Model download / network fetching (user pre-downloads the model)
- Batch/offline recognition (streaming only)
- Language selector UI
- Mobile / WASM targets
- Non-streaming model types (offline Whisper, etc.)

---

## 3. File Structure

```
libs/asr_widget/
├── Cargo.toml
└── src/
    ├── lib.rs                  # pub re-exports
    └── sherpa_asr_input.rs     # widget, shared state, recognition loop
```

**Updated files:**
```
examples/speech_to_text/
├── Cargo.toml                  # remove makepad-voice, add makepad-asr-widget
└── src/
    ├── main.rs                 # import SherpaAsrInput/SherpaAsrInputAction
    └── speech_input.rs         # DELETED
```

**Workspace root `Cargo.toml`:** add `"libs/asr_widget"` to the `[workspace] members` array.

### `libs/asr_widget/Cargo.toml`

```toml
[package]
name = "makepad-asr-widget"
version = "0.1.0"
authors = ["Makepad <info@makepad.nl>"]
edition = "2021"
description = "Standalone Makepad widget for on-device streaming ASR via sherpa-onnx"
license = "MIT OR Apache-2.0"

[dependencies]
makepad-widgets = { path = "../../widgets", version = "2.0.0" }
sherpa-onnx     = "1"
```

---

## 4. State Machine

```
Idle ──(mic click, model loaded)──► Recording
Recording ──(mic click)──────────► Idle       (reset stream, emit RecordingStopped)
Recording ──(endpoint detected on timer)──► Recording  (emit FinalResult, reset stream in-place, continue)
Any ──(model_dir ≠ model_dir_loaded, detected on timer tick)──► reload recognizer → Idle
```

**Note:** The `model_dir` change transition is polled on each timer tick (comparing `model_dir` vs `model_dir_loaded`). It is NOT event-driven — no `LiveHook` is needed.

Mic button is disabled when `model_dir` is empty or the recognizer failed to load.
No `Connecting`/`Closing` states — recognition is fully local with no network round-trip.

---

## 5. Shared State (`Arc<SherpaAsrShared>`)

```rust
pub struct SherpaAsrShared {
    pub pending_samples: Mutex<Vec<f32>>,   // PCM f32 at 16 kHz, drained each timer tick
    pub recent_samples:  Mutex<Vec<f32>>,   // last 1600 samples (100ms) for amplitude
    pub is_recording:    AtomicBool,
}
```

The audio callback (any thread) writes to `pending_samples` and `recent_samples`. The widget timer (UI thread) reads from both. The recognizer and stream live entirely on the UI thread — no locking required for the inference path.

---

## 6. Widget Struct (`SherpaAsrInput`)

```rust
#[derive(Script, ScriptHook, Widget)]
pub struct SherpaAsrInput {
    // --- Required Makepad Widget fields ---
    #[uid]     uid:    WidgetUid,
    #[source]  source: ScriptObjectRef,
    #[walk]    walk:   Walk,
    #[layout]  layout: Layout,

    // --- Live fields (DSL-configurable) ---
    #[live] pub model_dir:        String,   // path to sherpa-onnx model directory
    #[live] pub accent_color:     Vec4,     // default #FF6600
    #[live(40.0)] pub mic_button_size: f64,

    // --- Inner widgets (populated by DSL via #[find]) ---
    #[find] #[redraw] #[live] text_input:    WidgetRef,  // TextInput for final text
    #[find] #[redraw] #[live] interim_label: WidgetRef,  // Label for interim text (gray)

    // --- Draw state ---
    #[redraw] #[live] draw_mic:     DrawMicButton,
    #[redraw] #[live] draw_spinner: DrawSpinner,
    #[redraw] #[live] draw_bg:      DrawQuad,

    #[live(true)] #[visible] visible: bool,

    // --- Rust-only runtime state ---
    #[rust] recognizer:        Option<OnlineRecognizer>,  // sherpa-onnx recognizer (Send+Sync)
    #[rust] stream:            Option<OnlineStream>,       // current utterance stream (UI thread only)
    #[rust] shared:            Option<Arc<SherpaAsrShared>>,
    #[rust] model_dir_loaded:  String,   // last path successfully loaded (polled change detection)
    #[rust] current_amplitude: f32,
    #[rust] update_timer:      Timer,
    #[rust] mic_area:          Area,
}
```

**`#[uid]` vs `#[rust]`:** the `uid` field must use `#[uid]`, not `#[rust]`, so the derive macro assigns the correct `WidgetUid`. Using `#[rust]` would leave it zero, breaking all action dispatch.

---

## 7. Widget Initialization

The widget is initialized by the host app in `handle_startup`:

```rust
// In App::handle_startup:
let shared = Arc::new(SherpaAsrShared {
    pending_samples: Mutex::new(Vec::new()),
    recent_samples:  Mutex::new(Vec::new()),
    is_recording:    AtomicBool::new(false),
});
if let Some(mut w) = self.ui.widget(cx, ids!(asr_input)).borrow_mut::<SherpaAsrInput>() {
    w.init(cx, shared.clone());
}
self.shared = Some(shared);
```

`SherpaAsrInput::init(cx, shared)` stores the `Arc`, starts the 30fps timer, and sets `model_dir` from any value already applied via DSL. The recognizer is loaded lazily on the first timer tick where `model_dir` is non-empty and differs from `model_dir_loaded`.

**Setting `model_dir` at runtime** (e.g., from an env var):

```rust
// Option A — apply_over with live! macro:
self.ui.widget(cx, ids!(asr_input))
    .apply_over(cx, live!{ model_dir: (path_string) });

// Option B — expose a setter on the widget:
// pub fn set_model_dir(&mut self, cx: &mut Cx, path: &str)
// which calls self.model_dir = path.to_string(); self.redraw(cx);
```

The spec requires the widget to expose `pub fn set_model_dir(&mut self, cx: &mut Cx, path: &str)` for convenience (used by the updated `examples/speech_to_text`).

---

## 8. Actions

```rust
#[derive(Clone, Debug, Default)]
pub enum SherpaAsrInputAction {
    #[default]
    None,
    RecordingStarted,
    RecordingStopped,
    InterimResult(String),
    FinalResult(String),
    ModelLoadError(String),
}
```

---

## 9. Model Auto-Detection

Given `model_dir`, the widget scans for ONNX files with `std::fs::read_dir` and infers the model type by filename pattern:

| Files found in directory | Model type | Config field populated |
|---|---|---|
| `encoder*.onnx` + `decoder*.onnx` + `joiner*.onnx` + `tokens.txt` | Transducer (Zipformer, LSTM-RNN-T) | `OnlineModelConfig::transducer` |
| `encoder*.onnx` + `ctc*.onnx` + `tokens.txt` | Streaming CTC | `OnlineModelConfig::streaming_ctc` (field name: `ctc`) |
| Anything else | Error | — |

**Transducer config:**
```rust
OnlineRecognizerConfig {
    feat_config: FeatureConfig { sample_rate: 16000, feature_dim: 80 },
    model_config: OnlineModelConfig {
        transducer: OnlineTransducerModelConfig {
            encoder: encoder_path,
            decoder: decoder_path,
            joiner:  joiner_path,
        },
        tokens: tokens_path,
        num_threads: 1,
        ..Default::default()
    },
    ..Default::default()
}
```

**Streaming CTC config:**
```rust
OnlineRecognizerConfig {
    feat_config: FeatureConfig { sample_rate: 16000, feature_dim: 80 },
    model_config: OnlineModelConfig {
        ctc: OnlineCtcModelConfig {
            model: ctc_onnx_path,
        },
        tokens: tokens_path,
        num_threads: 1,
        ..Default::default()
    },
    ..Default::default()
}
```

Model loading is **synchronous** on the UI thread (< 500ms for Zipformer-small). It is triggered on the timer tick when `model_dir ≠ model_dir_loaded`.

---

## 10. Timer Tick Logic (30fps)

Called from `Widget::handle_event` on `Event::Timer`:

```
1. If model_dir ≠ model_dir_loaded:
     if is_recording: stop_recording(), emit RecordingStopped
     try load_recognizer(model_dir)
       → success: store recognizer, model_dir_loaded = model_dir
       → failure: emit ModelLoadError, clear recognizer, model_dir_loaded = model_dir

2. If is_recording and recognizer is Some and stream is Some:
   a. samples = drain pending_samples
   b. stream.accept_waveform(16000, &samples)   // &mut stream
   c. while recognizer.is_ready(&stream):
        recognizer.decode(&mut stream)           // requires &mut OnlineStream
   d. if recognizer.is_endpoint(&stream):
        text = recognizer.get_result(&stream).text.clone()
        emit FinalResult(text)
        recognizer.reset(&mut stream)            // resets stream IN-PLACE for next utterance
      else if !recognizer.get_result(&stream).text.is_empty():
        emit InterimResult(recognizer.get_result(&stream).text.clone())

3. Smooth amplitude from recent_samples → update draw_mic.amplitude
4. redraw(cx)
```

**`&mut OnlineStream` requirement:** `decode`, `reset`, and `accept_waveform` all require `&mut OnlineStream`. `is_ready`, `is_endpoint`, and `get_result` take `&OnlineStream`. The stream is stored as `Option<OnlineStream>` in the widget and accessed via `self.stream.as_mut().unwrap()`.

**Stream lifecycle:** `OnlineStream` is created via `recognizer.create_stream()` at recording start. On each endpoint, `recognizer.reset(&mut stream)` resets the existing stream **in-place** — a new stream is NOT created. The stream is dropped only when recording stops.

**Unprocessed audio between ticks:** `accept_waveform` pushes samples into the stream's internal queue. Any samples not yet decoded (i.e., `is_ready` returned false) remain in the stream's buffer and are decoded on the next tick. No extra buffering is needed in the widget.

---

## 11. Audio Processing

Same resampling logic as `speech_to_text` and `doubao_asr`:
- Linear interpolation resample to 16 kHz mono
- `recent_samples` capped at 1600 samples (100ms)
- `pending_samples` accumulates only when `is_recording == true`

```rust
pub fn process_audio_input(
    shared: &Arc<SherpaAsrShared>,
    info: AudioInfo,
    buf: &AudioBuffer,
)
```

---

## 12. Updated `examples/speech_to_text`

`main.rs` changes:
- Add `use makepad_asr_widget::{SherpaAsrInput, SherpaAsrInputAction, SherpaAsrShared, process_audio_input};`
- `script_mod!` block: register shaders + widget (see Section 13 for required DSL)
- In `handle_startup`:
  - Read `MAKEPAD_ASR_MODEL_DIR` env var
  - Create `Arc<SherpaAsrShared>`, call `widget.init(cx, shared.clone())`
  - Call `widget.set_model_dir(cx, &model_dir_path)` if env var is set
- `handle_audio_devices`: wire `cx.audio_input()` with `process_audio_input`
- `handle_actions`: match `SherpaAsrInputAction` variants to update status label and TextInput

`speech_input.rs` is deleted entirely — all widget logic lives in `libs/asr_widget/`.

---

## 13. Draw Structs and Shader Registration

`DrawMicButton` and `DrawSpinner` are defined in `libs/asr_widget/src/sherpa_asr_input.rs` and made `pub`. The widget crate exports the Rust structs only.

**Every host app** must register the shaders in its own `script_mod!` block. This is a known limitation — DSL shader code is not portable across crates. The `examples/speech_to_text` `script_mod!` block must include:

```
set_type_default() do #(DrawMicButton::script_shader(vm)) { /* pixel shader */ }
set_type_default() do #(DrawSpinner::script_shader(vm))   { /* pixel shader */ }
mod.widgets.SherpaAsrInputBase = #(SherpaAsrInput::register_widget(vm))
mod.widgets.SherpaAsrInput = set_type_default() do mod.widgets.SherpaAsrInputBase {
    width: Fill
    height: Fit
    flow: Down
    spacing: 10
    accent_color: #FF6600
    mic_button_size: 40.0
    draw_spinner.color: #FF6600

    text_input := TextInput {
        width: Fill
        height: 50
        empty_text: "Type or speak..."
        draw_bg.border_color: #FF6600
        draw_bg.border_radius: 25.0
        padding: {left: 15, right: 55, top: 12, bottom: 12}
    }

    interim_label := Label {
        width: Fill
        height: Fit
        text: ""
        draw_text.color: #888888
        draw_text.text_style.font_size: 12
    }
}
```

The `interim_label` child is a `Label` widget with id `interim_label`. The `#[find]` attribute on `SherpaAsrInput::interim_label` locates it by this id at layout time.

---

## 14. Error Handling

| Scenario | Behavior |
|---|---|
| `model_dir` empty | Mic button disabled; no action emitted |
| Directory not found | `ModelLoadError("model not found: <path>")` emitted; mic disabled |
| Directory found, no recognized ONNX layout | `ModelLoadError("unsupported model layout in <path>")` |
| sherpa-onnx returns error on load | `ModelLoadError(<sherpa error message>)` |
| `model_dir` changes during recording | Stop recording, emit `RecordingStopped`, reload recognizer |
| Audio arrives before model loaded | Samples discarded (`is_recording` is false) |

---

## 15. Recommended Test Model

```bash
# English Zipformer streaming model (~80MB)
wget https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-streaming-zipformer-en-2023-06-26.tar.bz2
tar xf sherpa-onnx-streaming-zipformer-en-2023-06-26.tar.bz2

export MAKEPAD_ASR_MODEL_DIR=$(pwd)/sherpa-onnx-streaming-zipformer-en-2023-06-26
cargo run -p makepad-example-speech-to-text
```

---

## 16. Manual Testing Plan

1. **No model dir:** Run without env var → mic disabled, no crash
2. **Wrong path:** Set invalid path → `ModelLoadError` in status label, mic disabled
3. **Happy path:** Set valid Zipformer path → click mic → speak English → interim gray text → endpoint reached → final text appended to TextInput
4. **Multi-utterance:** Keep recording past first endpoint → new utterance accumulates without stopping mic
5. **Model reload:** Change env var path and restart → new model loads, different model works
