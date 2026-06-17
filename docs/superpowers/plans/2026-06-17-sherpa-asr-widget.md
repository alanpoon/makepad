# SherpaAsrInput Widget Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create `libs/asr_widget/` — a standalone Makepad widget crate that provides on-device streaming ASR via `sherpa-onnx`, then update `examples/speech_to_text` to use it in place of `makepad-voice`.

**Architecture:** A `SherpaAsrInput` widget holds an `OnlineRecognizer` and `OnlineStream` directly on the UI thread. Audio samples flow from the audio callback via `Arc<SherpaAsrShared>` into a `pending_samples` buffer. The widget's 30fps timer drains that buffer, feeds it to the recognizer, and emits `InterimResult`/`FinalResult` actions. Model auto-detection reads the directory for ONNX files and picks Transducer or CTC config automatically.

**Tech Stack:** Rust, Makepad (makepad-widgets 2.0.0), sherpa-onnx 1.x (ONNX-based on-device streaming ASR).

**Spec:** `docs/superpowers/specs/2026-06-17-sherpa-asr-widget-design.md`

**Reference:** `examples/speech_to_text/src/speech_input.rs` — existing widget being replaced.

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `libs/asr_widget/Cargo.toml` | Create | Package metadata, deps |
| `libs/asr_widget/src/lib.rs` | Create | Re-exports |
| `libs/asr_widget/src/sherpa_asr_input.rs` | Create | All widget logic |
| `Cargo.toml` (workspace root) | Modify | Add `"libs/asr_widget"` to members |
| `examples/speech_to_text/Cargo.toml` | Modify | Swap deps |
| `examples/speech_to_text/src/speech_input.rs` | Delete | Replaced by `libs/asr_widget` |
| `examples/speech_to_text/src/main.rs` | Rewrite | Use SherpaAsrInput |

---

## Chunk 1: Library Skeleton, Shared State, Draw Structs, Action Enum

### Task 1: Create `libs/asr_widget/` project skeleton

**Files:**
- Create: `libs/asr_widget/Cargo.toml`
- Create: `libs/asr_widget/src/lib.rs`
- Create: `libs/asr_widget/src/sherpa_asr_input.rs` (stub)
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create `libs/asr_widget/Cargo.toml`**

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

- [ ] **Step 2: Create `libs/asr_widget/src/lib.rs`**

```rust
pub use makepad_widgets;

pub mod sherpa_asr_input;
pub use sherpa_asr_input::*;
```

- [ ] **Step 3: Create `libs/asr_widget/src/sherpa_asr_input.rs` (stub)**

```rust
// SherpaAsrInput — streaming on-device ASR widget using sherpa-onnx
use makepad_widgets::*;
```

- [ ] **Step 4: Add `"libs/asr_widget"` to workspace root `Cargo.toml`**

In `/Users/alanpoon/Documents/rust/makepad/Cargo.toml`, find the line `"libs/voice"` and add the new member immediately before it:

```toml
    "libs/asr_widget",
    "libs/voice",
```

- [ ] **Step 5: Verify compilation**

```bash
cd /Users/alanpoon/Documents/rust/makepad
cargo check -p makepad-asr-widget
```

Expected: `Finished` with no errors. `sherpa-onnx` will download its prebuilt native library on first build (requires internet access). If it fails to download, see sherpa-onnx-sys README for offline setup.

- [ ] **Step 6: Commit**

```bash
git add libs/asr_widget/ Cargo.toml
git commit -m "feat(asr-widget): add project skeleton"
```

---

### Task 2: `SherpaAsrShared` + `process_audio_input`

**Files:**
- Modify: `libs/asr_widget/src/sherpa_asr_input.rs`

- [ ] **Step 1: Replace the stub with shared state and audio helpers**

Replace the entire contents of `libs/asr_widget/src/sherpa_asr_input.rs` with:

```rust
use makepad_widgets::*;
use makepad_widgets::makepad_platform::audio::{AudioInfo, AudioBuffer};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const ASR_SAMPLE_RATE: f64 = 16000.0;
const MAX_RECENT_SAMPLES: usize = 1600; // 100ms at 16kHz

// ─── Shared state (audio thread ↔ UI timer) ──────────────────────────────────

pub struct SherpaAsrShared {
    pub pending_samples: Mutex<Vec<f32>>,
    pub recent_samples:  Mutex<Vec<f32>>,
    pub is_recording:    AtomicBool,
}

impl SherpaAsrShared {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            pending_samples: Mutex::new(Vec::new()),
            recent_samples:  Mutex::new(Vec::new()),
            is_recording:    AtomicBool::new(false),
        })
    }

    pub fn drain_pending(&self) -> Vec<f32> {
        std::mem::take(&mut *self.pending_samples.lock().unwrap())
    }

    pub fn calculate_amplitude(&self) -> f32 {
        let recent = self.recent_samples.lock().unwrap();
        if recent.is_empty() { return 0.0; }
        let rms = (recent.iter().map(|s| s * s).sum::<f32>() / recent.len() as f32).sqrt();
        (rms * 30.0).min(1.0)
    }
}

// ─── Audio resampler ──────────────────────────────────────────────────────────

fn resample_to_16k_mono(input: &AudioBuffer, from_rate: f64) -> Vec<f32> {
    if input.frame_count() == 0 { return Vec::new(); }
    let ratio = ASR_SAMPLE_RATE / from_rate;
    let new_len = ((input.frame_count() as f64 * ratio).round() as usize).max(1);
    let mut output = vec![0.0f32; new_len];
    let channel_count = input.channel_count().max(1) as f32;
    for i in 0..new_len {
        let src_pos = i as f64 / ratio;
        let src_idx = src_pos as usize;
        let frac = (src_pos - src_idx as f64) as f32;
        let mut s0 = 0.0f32;
        let mut s1 = 0.0f32;
        for ch in 0..input.channel_count() {
            s0 += input.channel(ch).get(src_idx).copied().unwrap_or(0.0);
            s1 += input.channel(ch).get(src_idx + 1).copied().unwrap_or(0.0);
        }
        s0 /= channel_count;
        s1 /= channel_count;
        if s1 == 0.0 { s1 = s0; }
        output[i] = s0 + (s1 - s0) * frac;
    }
    output
}

/// Call this from `cx.audio_input()` callback. Safe to call from any thread.
pub fn process_audio_input(
    shared: &Arc<SherpaAsrShared>,
    info: AudioInfo,
    input_buffer: &AudioBuffer,
) {
    let resampled = resample_to_16k_mono(input_buffer, info.sample_rate);
    {
        let mut recent = shared.recent_samples.lock().unwrap();
        recent.extend_from_slice(&resampled);
        let len = recent.len();
        if len > MAX_RECENT_SAMPLES {
            recent.drain(0..len - MAX_RECENT_SAMPLES);
        }
    }
    if shared.is_recording.load(Ordering::SeqCst) {
        shared.pending_samples.lock().unwrap().extend_from_slice(&resampled);
    }
}
```

- [ ] **Step 2: Check compilation**

```bash
cargo check -p makepad-asr-widget
```

Expected: no errors (AudioInfo/AudioBuffer import warnings are OK at this stage).

- [ ] **Step 3: Commit**

```bash
git add libs/asr_widget/src/sherpa_asr_input.rs
git commit -m "feat(asr-widget): add SherpaAsrShared and audio resampler"
```

---

### Task 3: `DrawMicButton` and `DrawSpinner` draw structs

**Files:**
- Modify: `libs/asr_widget/src/sherpa_asr_input.rs`

These are identical to those in `examples/speech_to_text/src/speech_input.rs`. The shaders are registered in the host app's `script_mod!` block, not here.

- [ ] **Step 1: Append draw structs to `sherpa_asr_input.rs`**

Append after `process_audio_input`:

```rust
// ─── Draw structs (shader registration in host app's script_mod!) ─────────────

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawMicButton {
    #[deref] pub draw_super:   DrawQuad,
    #[live]  pub is_recording: f32,
    #[live]  pub amplitude:    f32,
    #[live]  pub accent_color: Vec4,
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawSpinner {
    #[deref] pub draw_super: DrawQuad,
    #[live]  pub color:      Vec4,
    #[live]  pub time:       f32,
}
```

- [ ] **Step 2: Check compilation**

```bash
cargo check -p makepad-asr-widget
```

- [ ] **Step 3: Commit**

```bash
git add libs/asr_widget/src/sherpa_asr_input.rs
git commit -m "feat(asr-widget): add DrawMicButton and DrawSpinner structs"
```

---

### Task 4: `SherpaAsrInputAction` enum

**Files:**
- Modify: `libs/asr_widget/src/sherpa_asr_input.rs`

- [ ] **Step 1: Append the action enum**

Append after the draw structs:

```rust
// ─── Actions ──────────────────────────────────────────────────────────────────

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

- [ ] **Step 2: Check compilation**

```bash
cargo check -p makepad-asr-widget
```

- [ ] **Step 3: Commit**

```bash
git add libs/asr_widget/src/sherpa_asr_input.rs
git commit -m "feat(asr-widget): add SherpaAsrInputAction enum"
```

---

## Chunk 2: Widget Struct, Model Loading, Initialization

### Task 5: `SherpaAsrInput` widget struct

**Files:**
- Modify: `libs/asr_widget/src/sherpa_asr_input.rs`

- [ ] **Step 1: Add sherpa-onnx import and widget struct**

At the top of the file, add the sherpa-onnx import after the existing `use` statements:

```rust
use sherpa_onnx::{OnlineRecognizer, OnlineStream};
```

Then append the widget struct after the action enum:

```rust
// ─── Widget ───────────────────────────────────────────────────────────────────

#[derive(Script, ScriptHook, Widget)]
pub struct SherpaAsrInput {
    // Required Makepad widget fields
    #[uid]    uid:    WidgetUid,
    #[source] source: ScriptObjectRef,
    #[walk]   walk:   Walk,
    #[layout] layout: Layout,

    // Live fields (DSL-configurable)
    #[live] pub model_dir:        String,
    #[live] pub accent_color:     Vec4,
    #[live(40.0)] pub mic_button_size: f64,

    // Inner widgets found by id in the DSL
    #[find] #[redraw] #[live] text_input:    WidgetRef,
    #[find] #[redraw] #[live] interim_label: WidgetRef,

    // Draw state
    #[redraw] #[live] draw_mic:     DrawMicButton,
    #[redraw] #[live] draw_spinner: DrawSpinner,
    #[redraw] #[live] draw_bg:      DrawQuad,

    #[live(true)] #[visible] visible: bool,

    // Rust-only runtime state
    #[rust] recognizer:        Option<OnlineRecognizer>,
    #[rust] stream:            Option<OnlineStream>,
    #[rust] shared:            Option<Arc<SherpaAsrShared>>,
    #[rust] model_dir_loaded:  String,
    #[rust] current_amplitude: f32,
    #[rust] update_timer:      Timer,
    #[rust] mic_area:          Area,
}
```

- [ ] **Step 2: Check compilation**

```bash
cargo check -p makepad-asr-widget
```

Expected: may warn about unused `OnlineRecognizer`/`OnlineStream` — that's fine.

- [ ] **Step 3: Commit**

```bash
git add libs/asr_widget/src/sherpa_asr_input.rs
git commit -m "feat(asr-widget): add SherpaAsrInput widget struct"
```

---

### Task 6: Model auto-detection and `load_recognizer`

**Files:**
- Modify: `libs/asr_widget/src/sherpa_asr_input.rs`

- [ ] **Step 1: Verify the sherpa-onnx API available in this workspace**

After adding the dependency in Task 1, run:

```bash
cd /Users/alanpoon/Documents/rust/makepad
cargo doc -p makepad-asr-widget --no-deps 2>&1 | head -30
```

Check that `OnlineRecognizer`, `OnlineRecognizerConfig`, `OnlineModelConfig`, `OnlineTransducerModelConfig`, `FeatureConfig` are in scope. The exact field names may differ slightly from the spec — adjust the implementation below to match what `cargo doc` shows.

- [ ] **Step 2: Add `load_recognizer` function**

Append after the action enum, before the widget struct:

```rust
// ─── Model loading ────────────────────────────────────────────────────────────

/// Scan model_dir for ONNX files and build an OnlineRecognizer.
/// Supports Transducer (encoder+decoder+joiner) and streaming CTC (encoder+ctc) layouts.
fn load_recognizer(model_dir: &str) -> Result<OnlineRecognizer, String> {
    use sherpa_onnx::{
        OnlineRecognizerConfig, OnlineModelConfig,
        OnlineTransducerModelConfig, FeatureConfig,
    };
    use std::fs;

    let entries = fs::read_dir(model_dir)
        .map_err(|_| format!("model not found: {}", model_dir))?;

    let mut encoder = None;
    let mut decoder = None;
    let mut joiner  = None;
    let mut ctc     = None;
    let mut tokens  = None;

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path().to_string_lossy().to_string();
        if name.starts_with("encoder") && name.ends_with(".onnx")      { encoder = Some(path); }
        else if name.starts_with("decoder") && name.ends_with(".onnx") { decoder = Some(path); }
        else if name.starts_with("joiner")  && name.ends_with(".onnx") { joiner  = Some(path); }
        else if name.starts_with("ctc")     && name.ends_with(".onnx") { ctc     = Some(path); }
        else if name == "tokens.txt"                                    { tokens  = Some(path); }
    }

    let tokens = tokens.ok_or_else(||
        format!("unsupported model layout in {}", model_dir)
    )?;

    let model_config = if let (Some(enc), Some(dec), Some(joi)) = (encoder, decoder, joiner) {
        OnlineModelConfig {
            transducer: OnlineTransducerModelConfig {
                encoder: enc,
                decoder: dec,
                joiner:  joi,
            },
            tokens,
            num_threads: 1,
            ..Default::default()
        }
    } else if let Some(ctc_path) = ctc {
        // NOTE: The CTC field name may be `ctc` or `streaming_ctc` depending on
        // the sherpa-onnx version. Check `cargo doc` output and adjust.
        OnlineModelConfig {
            ctc: sherpa_onnx::OnlineCtcModelConfig {
                model: ctc_path,
            },
            tokens,
            num_threads: 1,
            ..Default::default()
        }
    } else {
        return Err(format!("unsupported model layout in {}", model_dir));
    };

    let config = OnlineRecognizerConfig {
        feat_config: FeatureConfig { sample_rate: 16000, feature_dim: 80 },
        model_config,
        enable_endpoint_detection: 1,
        ..Default::default()
    };

    // OnlineRecognizer::new may panic or return Result depending on sherpa-onnx version.
    // If it panics on bad config, wrap with std::panic::catch_unwind if needed.
    Ok(OnlineRecognizer::new(&config))
}
```

> **Implementer note:** `OnlineRecognizer::new` may return `Self` (not `Result<Self>`). If so, wrap the call in `std::panic::catch_unwind` to catch bad-model panics and convert to `Err(String)`. Check the actual API via `cargo doc`.

- [ ] **Step 3: Check compilation**

```bash
cargo check -p makepad-asr-widget
```

Fix any field name mismatches by consulting `cargo doc -p makepad-asr-widget --no-deps`.

- [ ] **Step 4: Commit**

```bash
git add libs/asr_widget/src/sherpa_asr_input.rs
git commit -m "feat(asr-widget): add model auto-detection and load_recognizer"
```

---

### Task 7: Widget `init`, `set_model_dir`, `handle_action`, and `WidgetMatchEvent`

**Files:**
- Modify: `libs/asr_widget/src/sherpa_asr_input.rs`

- [ ] **Step 1: Append `impl SherpaAsrInput` block**

Append after the widget struct:

```rust
impl SherpaAsrInput {
    /// Called by the host app in `handle_startup`. Stores the shared audio state
    /// and starts the 30fps update timer.
    pub fn init(&mut self, cx: &mut Cx, shared: Arc<SherpaAsrShared>) {
        self.shared = Some(shared);
        self.update_timer = cx.start_interval(0.033);
    }

    /// Set the model directory at runtime (e.g., from an env var).
    /// The recognizer will be loaded on the next timer tick.
    pub fn set_model_dir(&mut self, cx: &mut Cx, path: &str) {
        self.model_dir = path.to_string();
        self.redraw(cx);
    }

    /// Extract this widget's action from an `Actions` list.
    /// Use in the host app's `handle_actions`.
    pub fn handle_action(&self, actions: &Actions) -> Option<SherpaAsrInputAction> {
        actions
            .find_widget_action(self.widget_uid())
            .map(|a| a.cast::<SherpaAsrInputAction>())
    }
}

impl WidgetMatchEvent for SherpaAsrInput {
    fn handle_actions(&mut self, _cx: &mut Cx, _actions: &Actions, _scope: &mut Scope) {}
}
```

- [ ] **Step 2: Check compilation**

```bash
cargo check -p makepad-asr-widget
```

- [ ] **Step 3: Commit**

```bash
git add libs/asr_widget/src/sherpa_asr_input.rs
git commit -m "feat(asr-widget): add widget init, set_model_dir, handle_action"
```

---

## Chunk 3: Widget Drawing and Event Handling

### Task 8: `Widget::draw_walk`

**Files:**
- Modify: `libs/asr_widget/src/sherpa_asr_input.rs`

- [ ] **Step 1: Append `impl Widget for SherpaAsrInput` with `draw_walk`**

Append:

```rust
impl Widget for SherpaAsrInput {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.visible { return DrawStep::done(); }

        self.draw_mic.accent_color = self.accent_color;
        let button_walk = Walk::fixed(self.mic_button_size, self.mic_button_size);

        // Show spinner while model is loading (model_dir set but recognizer not yet loaded)
        let is_loading = !self.model_dir.is_empty()
            && self.model_dir != self.model_dir_loaded
            && self.recognizer.is_none();

        if is_loading {
            self.draw_spinner.time = cx.time() as f32;
            let _ = self.draw_spinner.draw_walk(cx, button_walk);
        } else {
            let is_recording = self.shared.as_ref()
                .map(|s| s.is_recording.load(Ordering::SeqCst))
                .unwrap_or(false);
            self.draw_mic.is_recording = if is_recording { 1.0 } else { 0.0 };
            self.draw_mic.amplitude = self.current_amplitude;
            let _ = self.draw_mic.draw_walk(cx, button_walk);
            self.mic_area = self.draw_mic.area();
        }

        let _ = self.text_input.draw_walk(cx, scope, walk);
        let _ = self.interim_label.draw_walk(cx, scope, Walk::default());

        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        // implemented in Task 9
        let _ = (cx, event);
    }
}
```

- [ ] **Step 2: Check compilation**

```bash
cargo check -p makepad-asr-widget
```

- [ ] **Step 3: Commit**

```bash
git add libs/asr_widget/src/sherpa_asr_input.rs
git commit -m "feat(asr-widget): implement draw_walk"
```

---

### Task 9: `handle_event` — timer tick recognition loop + mic click

**Files:**
- Modify: `libs/asr_widget/src/sherpa_asr_input.rs`

This is the core of the widget. Replace the stub `handle_event` body with the full implementation.

- [ ] **Step 1: Replace the `handle_event` body inside `impl Widget for SherpaAsrInput`**

Replace:
```rust
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        // implemented in Task 9
        let _ = (cx, event);
    }
```

With:
```rust
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if !self.visible { return; }

        if let Event::Timer(te) = event {
            if self.update_timer.is_timer(te).is_some() {
                self.timer_tick(cx);
            }
        }

        let model_ready = self.recognizer.is_some();
        if model_ready {
            if let Hit::FingerDown(_) = event.hits(cx, self.mic_area) {
                self.toggle_recording(cx);
            }
        }
    }
```

- [ ] **Step 2: Add `timer_tick` and `toggle_recording` to `impl SherpaAsrInput`**

Add to the existing `impl SherpaAsrInput` block:

```rust
    fn timer_tick(&mut self, cx: &mut Cx) {
        // ── 1. Detect model_dir change and (re)load recognizer ─────────────────
        if self.model_dir != self.model_dir_loaded {
            // Stop any ongoing recording
            if let Some(shared) = &self.shared {
                if shared.is_recording.load(Ordering::SeqCst) {
                    shared.is_recording.store(false, Ordering::SeqCst);
                    self.stream = None;
                    cx.widget_action(self.widget_uid(), SherpaAsrInputAction::RecordingStopped);
                }
            }
            if self.model_dir.is_empty() {
                self.recognizer = None;
            } else {
                match load_recognizer(&self.model_dir) {
                    Ok(rec) => {
                        self.recognizer = Some(rec);
                    }
                    Err(e) => {
                        self.recognizer = None;
                        cx.widget_action(self.widget_uid(), SherpaAsrInputAction::ModelLoadError(e));
                    }
                }
            }
            self.model_dir_loaded = self.model_dir.clone();
        }

        // ── 2. Recognition loop ─────────────────────────────────────────────────
        let is_recording = self.shared.as_ref()
            .map(|s| s.is_recording.load(Ordering::SeqCst))
            .unwrap_or(false);

        if is_recording {
            if let (Some(rec), Some(stream), Some(shared)) =
                (&self.recognizer, self.stream.as_mut(), &self.shared)
            {
                let samples = shared.drain_pending();
                if !samples.is_empty() {
                    stream.accept_waveform(16000, &samples);
                }
                // Decode all available frames
                while rec.is_ready(stream) {
                    rec.decode(stream);
                }
                let result_text = rec.get_result(stream).text.clone();
                if rec.is_endpoint(stream) {
                    self.interim_label.set_text(cx, "");
                    cx.widget_action(
                        self.widget_uid(),
                        SherpaAsrInputAction::FinalResult(result_text),
                    );
                    rec.reset(stream);
                } else if !result_text.is_empty() {
                    // Update interim_label directly
                    self.interim_label.set_text(cx, &result_text);
                    cx.widget_action(
                        self.widget_uid(),
                        SherpaAsrInputAction::InterimResult(result_text),
                    );
                }
            }
        } else {
            // Clear interim text when not recording
            if !self.interim_label.text().is_empty() {
                self.interim_label.set_text(cx, "");
            }
        }

        // ── 3. Update amplitude visualization ──────────────────────────────────
        let amplitude = self.shared.as_ref()
            .map(|s| s.calculate_amplitude())
            .unwrap_or(0.0);
        self.current_amplitude = self.current_amplitude * 0.7 + amplitude * 0.3;

        self.redraw(cx);
    }

    fn toggle_recording(&mut self, cx: &mut Cx) {
        let shared = match &self.shared { Some(s) => s.clone(), None => return };
        let rec    = match &self.recognizer { Some(r) => r, None => return };

        if shared.is_recording.load(Ordering::SeqCst) {
            // Stop recording
            shared.is_recording.store(false, Ordering::SeqCst);
            self.stream = None;
            self.interim_label.set_text(cx, "");
            cx.widget_action(self.widget_uid(), SherpaAsrInputAction::RecordingStopped);
        } else {
            // Start recording: create a fresh stream
            // NOTE: create_stream() may return OnlineStream directly or Result<OnlineStream>.
            // Check cargo doc and adjust accordingly. If it returns Result, unwrap with error emit.
            let stream = rec.create_stream();
            self.stream = Some(stream);
            self.interim_label.set_text(cx, "");
            shared.pending_samples.lock().unwrap().clear();
            shared.is_recording.store(true, Ordering::SeqCst);
            cx.widget_action(self.widget_uid(), SherpaAsrInputAction::RecordingStarted);
        }
        self.redraw(cx);
    }
```

> **Implementer note on `rec.decode(stream)` and `rec.reset(stream)`:** The sherpa-onnx Rust API takes `&mut OnlineStream` for these methods. `stream` above is `&mut OnlineStream` from `self.stream.as_mut()`, so the calls should compile. If they take `&OnlineStream`, remove the `&mut`.
>
> **Implementer note on `create_stream()`:** May return `OnlineStream` directly (not `Result`). If so, remove the `let stream = rec.create_stream();` binding and just use it directly. If it returns `Result`, handle the error with `cx.widget_action(..., SherpaAsrInputAction::ModelLoadError(...))`.

- [ ] **Step 3: Check compilation**

```bash
cargo check -p makepad-asr-widget
```

Fix borrow errors iteratively. The most common issue: `rec` borrows `self.recognizer` while `self.stream` is also borrowed mutably. The fix is to extract `rec` as a local reference after the stream borrow pattern — Rust understands disjoint field borrows.

- [ ] **Step 4: Commit**

```bash
git add libs/asr_widget/src/sherpa_asr_input.rs
git commit -m "feat(asr-widget): implement timer tick recognition loop and mic toggle"
```

---

## Chunk 4: Update `examples/speech_to_text`

### Task 10: Update `Cargo.toml`, delete `speech_input.rs`

**Files:**
- Modify: `examples/speech_to_text/Cargo.toml`
- Delete: `examples/speech_to_text/src/speech_input.rs`

- [ ] **Step 1: Rewrite `examples/speech_to_text/Cargo.toml`**

```toml
[package]
name = "makepad-example-speech-to-text"
version = "1.0.0"
authors = ["Makepad <info@makepad.nl>"]
edition = "2021"
description = "Makepad speech-to-text example with waveform visualizer"
license = "MIT OR Apache-2.0"

[dependencies]
makepad-widgets    = { path = "../../widgets", version = "2.0.0" }
makepad-asr-widget = { path = "../../libs/asr_widget" }
```

- [ ] **Step 2: Delete `speech_input.rs`**

```bash
rm /Users/alanpoon/Documents/rust/makepad/examples/speech_to_text/src/speech_input.rs
```

- [ ] **Step 3: Verify `cargo check` fails with expected error**

```bash
cargo check -p makepad-example-speech-to-text
```

Expected: errors about `mod speech_input` not found and missing `SpeechInput` — this confirms the old module is gone.

- [ ] **Step 4: Commit**

```bash
git add examples/speech_to_text/Cargo.toml
git rm examples/speech_to_text/src/speech_input.rs
git commit -m "refactor(speech_to_text): drop makepad-voice, remove speech_input.rs"
```

---

### Task 11: Rewrite `examples/speech_to_text/src/main.rs`

**Files:**
- Modify: `examples/speech_to_text/src/main.rs`

- [ ] **Step 1: Replace `main.rs` entirely**

```rust
pub use makepad_widgets;

use makepad_widgets::makepad_draw::CxMediaApi;
use makepad_widgets::*;

use makepad_asr_widget::{
    DrawMicButton, DrawSpinner, SherpaAsrInput, SherpaAsrInputAction,
    SherpaAsrShared, process_audio_input,
};

use std::sync::Arc;

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*

    // Microphone button shader
    set_type_default() do #(DrawMicButton::script_shader(vm)){
        ..mod.draw.DrawQuad
        is_recording: 0.0
        amplitude: 0.0
        accent_color: #FF6600

        pixel: fn() {
            let p = self.pos - vec2(0.5, 0.5)
            let r = length(p)

            let bg_radius = 0.42
            let bg_mask = clamp(1.0 - (r - bg_radius) * 80.0, 0.0, 1.0)

            let bg_off = vec3(0.25, 0.28, 0.30)
            let bg_on = self.accent_color.xyz
            let bg_color = bg_off.mix(bg_on, self.is_recording)

            let glow_radius = 0.48 + self.amplitude * 0.08
            let glow_mask = clamp(1.0 - (r - glow_radius) * 20.0, 0.0, 1.0) * self.is_recording * 0.5

            let level_y_start = -0.35
            let level_height = 0.05
            let level_spacing = 0.07
            let level_width = 0.25

            let mut in_level = false
            let mut level_color = vec3(0.2, 0.9, 0.5)

            if abs(p.x) < level_width && p.y > level_y_start && p.y < level_y_start + level_height && self.amplitude > 0.2 {
                in_level = true
            }
            if abs(p.x) < level_width && p.y > level_y_start + level_spacing && p.y < level_y_start + level_spacing + level_height && self.amplitude > 0.5 {
                in_level = true
            }
            if abs(p.x) < level_width && p.y > level_y_start + level_spacing * 2.0 && p.y < level_y_start + level_spacing * 2.0 + level_height && self.amplitude > 0.8 {
                in_level = true
                level_color = vec3(0.9, 0.7, 0.2)
            }

            let mic_width = 0.08
            let mic_height = 0.18
            let mic_top = 0.05
            let mic_body = abs(p.x) < mic_width && p.y > -mic_height && p.y < mic_top
            let mic_head = length(p - vec2(0.0, mic_top)) < mic_width
            let stand = abs(p.x) < 0.015 && p.y > -mic_height - 0.06 && p.y < -mic_height + 0.02
            let arc_dist = abs(length(p - vec2(0.0, -0.02)) - 0.12)
            let arc = arc_dist < 0.02 && p.y < -0.02
            let mic_icon = mic_body || mic_head || stand || arc

            let mut color = bg_color * 0.6
            let mut alpha = glow_mask
            color = color.mix(bg_color, bg_mask)
            alpha = max(alpha, bg_mask)

            if in_level && bg_mask > 0.5 { color = level_color }
            if mic_icon && bg_mask > 0.5 { color = vec3(1.0, 1.0, 1.0) }

            return vec4(color, alpha)
        }
    }

    // Spinner shader
    set_type_default() do #(DrawSpinner::script_shader(vm)){
        ..mod.draw.DrawQuad
        color: #FF6600
        time: 0.0

        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let stroke_width = 4.0
            let radius = min(self.rect_size.x * 0.5, self.rect_size.y * 0.5) - stroke_width * 0.5
            let center = self.rect_size * 0.5
            let rotation = self.time * 2.0 * PI * 1.2
            let rotation_cycles = rotation / (2.0 * PI)
            let arc_phase = modf(rotation_cycles * 0.5, 1.0)
            let expand_phase = clamp(arc_phase / 0.55, 0.0, 1.0)
            let contract_phase = clamp((arc_phase - 0.55) / 0.45, 0.0, 1.0)
            let cycle = expand_phase * (1.0 - contract_phase)
            let gap_ratio = mix(0.12, 0.92, cycle)
            let gap_radians = gap_ratio * 2.0 * PI
            let start_angle = rotation
            sdf.arc_round_caps(center.x center.y radius start_angle start_angle + 2.0 * PI - gap_radians stroke_width)
            return sdf.fill(self.color)
        }
    }

    // Register SherpaAsrInput widget
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
            draw_bg.color: #FFFFFF
            draw_bg.border_color: #FF6600
            draw_bg.border_size: 2.0
            draw_bg.border_radius: 25.0
            padding: {left: 15, right: 55, top: 12, bottom: 12}
            draw_text.text_style.font_size: 14
        }

        interim_label := Label {
            width: Fill
            height: Fit
            text: ""
            draw_text.color: #888888
            draw_text.text_style.font_size: 12
        }
    }

    let state = {
        status: "Ready — click microphone to start"
    }
    mod.state = state

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.title: "Speech to Text"
                window.inner_size: vec2(700, 250)
                body +: {
                    main_view := View{
                        width: Fill
                        height: Fill
                        flow: Down
                        spacing: 20
                        padding: 30
                        align: Center
                        draw_bg.color: #5A5A5A

                        status_label := Label{
                            text: "Ready — click microphone to start"
                            draw_text.text_style.font_size: 14
                            draw_text.color: #CCCCCC
                        }
                        speech_input := mod.widgets.SherpaAsrInput{}
                    }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]  ui:               WidgetRef,
    #[rust]  shared:           Option<Arc<SherpaAsrShared>>,
    #[rust]  audio_initialized: bool,
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        let shared = SherpaAsrShared::new();
        self.shared = Some(shared.clone());

        if let Some(mut w) = self.ui.widget(cx, ids!(speech_input)).borrow_mut::<SherpaAsrInput>() {
            w.init(cx, shared);
            // Load model from env var if set
            if let Ok(model_dir) = std::env::var("MAKEPAD_ASR_MODEL_DIR") {
                w.set_model_dir(cx, &model_dir);
            } else {
                // Show instructions if no model dir configured
            }
        }

        if std::env::var("MAKEPAD_ASR_MODEL_DIR").is_err() {
            self.ui.label(cx, ids!(status_label))
                .set_text(cx, "Set MAKEPAD_ASR_MODEL_DIR to a sherpa-onnx model directory");
        }

        cx.use_audio_inputs(&[]);
    }

    fn handle_audio_devices(&mut self, cx: &mut Cx, devices: &AudioDevicesEvent) {
        cx.use_audio_inputs(&devices.default_input());
        self.audio_initialized = true;
        self.wire_audio(cx);
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let action = self.ui.widget(cx, ids!(speech_input))
            .borrow::<SherpaAsrInput>()
            .and_then(|w| w.handle_action(actions));

        if let Some(action) = action {
            match action {
                SherpaAsrInputAction::RecordingStarted => {
                    self.ui.label(cx, ids!(status_label))
                        .set_text(cx, "Recording… click mic to stop");
                }
                SherpaAsrInputAction::RecordingStopped => {
                    self.ui.label(cx, ids!(status_label))
                        .set_text(cx, "Ready — click microphone to start");
                }
                SherpaAsrInputAction::InterimResult(_) => {
                    // interim text shown directly in interim_label by the widget
                }
                SherpaAsrInputAction::FinalResult(text) => {
                    let existing = self.ui.text_input(cx, ids!(speech_input.text_input)).text();
                    let new_text = if existing.is_empty() {
                        text
                    } else {
                        format!("{} {}", existing, text)
                    };
                    self.ui.text_input(cx, ids!(speech_input.text_input)).set_text(cx, &new_text);
                    self.ui.label(cx, ids!(status_label))
                        .set_text(cx, "Ready — click microphone to continue");
                }
                SherpaAsrInputAction::ModelLoadError(e) => {
                    self.ui.label(cx, ids!(status_label))
                        .set_text(cx, &format!("Model error: {}", e));
                }
                SherpaAsrInputAction::None => {}
            }
        }
    }
}

impl App {
    fn wire_audio(&mut self, cx: &mut Cx) {
        let shared = match &self.shared { Some(s) => s.clone(), None => return };
        cx.audio_input(0, move |info, buf| {
            process_audio_input(&shared, info, buf);
        });
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        crate::makepad_widgets::script_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        let _ = self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
```

- [ ] **Step 2: Check compilation**

```bash
cargo check -p makepad-example-speech-to-text
```

Fix errors iteratively:
- If `self.ui.label(cx, ids!(...)).set_text(...)` fails, try `self.ui.widget(cx, ids!(...)).set_text(cx, ...)`
- If `self.ui.text_input(cx, ids!(...)).text()` fails, use `.borrow::<TextInput>()` pattern

- [ ] **Step 3: Commit**

```bash
git add examples/speech_to_text/src/main.rs
git commit -m "refactor(speech_to_text): use SherpaAsrInput widget from libs/asr_widget"
```

---

### Task 12: Integration check and manual testing instructions

- [ ] **Step 1: Final clean compile check**

```bash
cd /Users/alanpoon/Documents/rust/makepad
cargo check -p makepad-asr-widget -p makepad-example-speech-to-text
```

Expected: 0 errors, only unused-variant warnings.

- [ ] **Step 2: Download a test model (English Zipformer, ~80MB)**

```bash
cd /tmp
wget https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-streaming-zipformer-en-2023-06-26.tar.bz2
tar xf sherpa-onnx-streaming-zipformer-en-2023-06-26.tar.bz2
```

- [ ] **Step 3: Test — no model dir (mic disabled)**

```bash
cd /Users/alanpoon/Documents/rust/makepad
cargo run -p makepad-example-speech-to-text
```

Expected: window opens, status shows "Set MAKEPAD_ASR_MODEL_DIR…", mic button is disabled (not clickable).

- [ ] **Step 4: Test — with model dir (happy path)**

```bash
export MAKEPAD_ASR_MODEL_DIR=/tmp/sherpa-onnx-streaming-zipformer-en-2023-06-26
cargo run -p makepad-example-speech-to-text
```

Expected:
1. App opens, model loads (< 1s)
2. Click mic → "Recording…" in status
3. Speak English → gray interim text appears below the text box
4. Endpoint detected → final text appended to white TextInput, interim clears
5. Click mic again → recording stops

- [ ] **Step 5: Test — wrong model path**

```bash
export MAKEPAD_ASR_MODEL_DIR=/nonexistent/path
cargo run -p makepad-example-speech-to-text
```

Expected: status shows "Model error: model not found: /nonexistent/path", mic disabled.

- [ ] **Step 6: Test — multi-utterance (keep recording past first endpoint)**

```bash
export MAKEPAD_ASR_MODEL_DIR=/tmp/sherpa-onnx-streaming-zipformer-en-2023-06-26
cargo run -p makepad-example-speech-to-text
```

Procedure: Click mic → speak a complete sentence → wait for endpoint → speak another sentence → wait for endpoint → click mic to stop.

Expected:
1. First sentence appears in TextInput after first endpoint; interim label clears
2. Second sentence is appended to TextInput after second endpoint (separated by space)
3. Mic never auto-stops between utterances; `RecordingStopped` is only emitted when mic is clicked

- [ ] **Step 7: Test — model reload (restart with a different model)**

```bash
export MAKEPAD_ASR_MODEL_DIR=/tmp/sherpa-onnx-streaming-zipformer-en-2023-06-26
cargo run -p makepad-example-speech-to-text
```

After the app is running, kill it, set a different `MAKEPAD_ASR_MODEL_DIR` (or set it to an invalid path), and restart. Expected: new model loads (or error shown), previous model is no longer used.

- [ ] **Step 8: Commit final state**

```bash
git add .
git commit -m "feat(asr-widget): complete SherpaAsrInput implementation and speech_to_text integration"
```
