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
Recording ──(mic click)──────────► Idle       (reset stream)
Recording ──(endpoint detected)──► Recording  (emit FinalResult, reset stream, continue)
Any ──(model_dir live field changes)──► reload recognizer → Idle
```

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
    // --- Live fields (DSL-configurable) ---
    #[live] pub model_dir:       String,   // path to sherpa-onnx model directory
    #[live] pub accent_color:    Vec4,     // default #FF6600
    #[live(40.0)] pub mic_button_size: f64,

    // --- Inner widgets ---
    #[find] #[redraw] #[live] text_input:    WidgetRef,
    #[find] #[redraw] #[live] interim_label: WidgetRef,

    // --- Draw state ---
    #[redraw] #[live] draw_mic:     DrawMicButton,
    #[redraw] #[live] draw_spinner: DrawSpinner,
    #[redraw] #[live] draw_bg:      DrawQuad,

    #[live(true)] #[visible] visible: bool,

    // --- Rust-only runtime state ---
    #[rust] recognizer:       Option<OnlineRecognizer>,   // sherpa-onnx recognizer
    #[rust] stream:           Option<OnlineStream>,        // current utterance stream
    #[rust] shared:           Option<Arc<SherpaAsrShared>>,
    #[rust] model_dir_loaded: String,   // last path successfully loaded (change detection)
    #[rust] current_amplitude: f32,
    #[rust] update_timer:     Timer,
    #[rust] mic_area:         Area,
    #[rust] uid:              WidgetUid,
}
```

---

## 7. Actions

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

## 8. Model Auto-Detection

Given `model_dir`, the widget globs for ONNX files and infers the model type:

| Files found in directory | Model type |
|---|---|
| `encoder*.onnx` + `decoder*.onnx` + `joiner*.onnx` + `tokens.txt` | Transducer (Zipformer, LSTM-RNN-T) |
| `encoder*.onnx` + `ctc*.onnx` + `tokens.txt` | Streaming CTC |
| Anything else | Error: unsupported layout |

Configuration built from discovered paths:

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

Model loading is **synchronous** on the UI thread (< 500ms for Zipformer-small). Triggered when `model_dir` changes (detected by comparing `model_dir` vs `model_dir_loaded` on each timer tick).

---

## 9. Timer Tick Logic (30fps)

Called from `Widget::handle_event` on `Event::Timer`:

```
1. If model_dir ≠ model_dir_loaded:
     try load_recognizer(model_dir)
       → success: store recognizer, model_dir_loaded = model_dir
       → failure: emit ModelLoadError, clear recognizer

2. If is_recording and recognizer is Some:
   a. drain pending_samples
   b. stream.accept_waveform(16000, &samples)
   c. while recognizer.is_ready(&stream): recognizer.decode(&stream)
   d. if recognizer.is_endpoint(&stream):
        text = recognizer.get_result(&stream).text
        emit FinalResult(text)
        recognizer.reset(&stream)   ← resets stream for next utterance
      else:
        emit InterimResult(recognizer.get_result(&stream).text)

3. Smooth amplitude from recent_samples → update draw_mic.amplitude
4. redraw(cx)
```

`OnlineStream` is created via `recognizer.create_stream()` at recording start and replaced on each endpoint.

---

## 10. Audio Processing

Same resampling logic as `speech_to_text` and `doubao_asr`:
- Linear interpolation to 16 kHz mono
- `recent_samples` capped at 1600 samples (100ms)
- `pending_samples` accumulates only when `is_recording == true`

Function: `pub fn process_audio_input(shared: &Arc<SherpaAsrShared>, info: AudioInfo, buf: &AudioBuffer)`

---

## 11. Updated `examples/speech_to_text`

`main.rs` is simplified significantly:
- Import `SherpaAsrInput`, `SherpaAsrInputAction`, `process_audio_input` from `makepad_asr_widget`
- `script_mod!` block: register shaders + widget, same UI layout as today
- `handle_startup`: init widget with `model_dir` from env var `MAKEPAD_ASR_MODEL_DIR`
- `handle_audio_devices`: wire `cx.audio_input()` with `process_audio_input`
- `handle_actions`: match `SherpaAsrInputAction` to update status label and TextInput

`speech_input.rs` is deleted entirely — all widget logic lives in `libs/asr_widget/`.

---

## 12. Draw Structs

`DrawMicButton` and `DrawSpinner` are moved from `speech_to_text` into `libs/asr_widget/src/sherpa_asr_input.rs` and made `pub`. Shader registration (`set_type_default()`) remains in the app's `script_mod!` block — the widget crate exports the Rust structs only, not DSL.

---

## 13. Error Handling

| Scenario | Behavior |
|---|---|
| `model_dir` empty | Mic button disabled; no action emitted |
| Directory not found | `ModelLoadError("model not found: <path>")` |
| Directory found, no recognized ONNX layout | `ModelLoadError("unsupported model layout in <path>")` |
| sherpa-onnx returns error on load | `ModelLoadError(<sherpa error message>)` |
| `model_dir` changes during recording | Stop recording, reload recognizer, emit `RecordingStopped` |
| Audio arrives before model loaded | Samples discarded (is_recording is false) |

---

## 14. Recommended Test Model

```bash
# English Zipformer streaming model (~80MB)
wget https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/\
sherpa-onnx-streaming-zipformer-en-2023-06-26.tar.bz2
tar xf sherpa-onnx-streaming-zipformer-en-2023-06-26.tar.bz2

export MAKEPAD_ASR_MODEL_DIR=$(pwd)/sherpa-onnx-streaming-zipformer-en-2023-06-26
cargo run -p makepad-example-speech-to-text
```

---

## 15. Manual Testing Plan

1. **No model dir:** Run without env var → mic disabled, no crash
2. **Wrong path:** Set invalid path → `ModelLoadError` in status label
3. **Happy path:** Set valid Zipformer path → click mic → speak English → interim text appears → endpoint reached → final text in TextInput
4. **Multi-utterance:** Keep recording past first endpoint → new utterance accumulates in TextInput
5. **Live reload:** Change `model_dir` mid-session → recording stops, new model loads, mic re-enables
