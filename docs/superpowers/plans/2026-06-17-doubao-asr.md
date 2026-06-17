# Doubao ASR Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Makepad desktop example that streams microphone audio to Doubao/VolcEngine's WebSocket ASR API and displays real-time Chinese transcription.

**Architecture:** Audio is captured via Makepad's `cx.audio_input()` callback, resampled to 16kHz mono, and accumulated in `Arc<DoubaoAsrState>`. A 30fps timer in the `DoubaoAsrInput` widget drains pending samples and sends them as binary PCM frames over a WebSocket opened with `cx.net.ws_open()`. `NetworkResponse` events (WsOpened / WsMessage / WsClosed / WsError) arrive via `Event::NetworkResponses` in the App's `handle_event` and drive a five-state machine (Idle → Connecting → Streaming → Closing → Idle/Error).

**Tech Stack:** Rust, Makepad (makepad-widgets 2.0.0), makepad-micro-serde (DeJson for response parsing), VolcEngine v2 binary-frame WebSocket ASR protocol, NSURLSession-backed TLS WebSocket on macOS.

**Spec:** `docs/superpowers/specs/2026-06-17-doubao-asr-example-design.md`

**Reference:** `examples/speech_to_text/src/` — the new example mirrors its widget structure.

---

## Chunk 1: Skeleton, State, and Audio Processing

### Task 1: Create project skeleton

**Files:**
- Create: `examples/doubao_asr/Cargo.toml`
- Create: `examples/doubao_asr/src/main.rs`
- Create: `examples/doubao_asr/src/doubao_asr.rs`
- Modify: `Cargo.toml` (workspace root) — add `"examples/doubao_asr"` to `members`

- [ ] **Step 1: Create `examples/doubao_asr/Cargo.toml`**

```toml
[package]
name = "makepad-example-doubao-asr"
version = "1.0.0"
authors = ["Makepad <info@makepad.nl>"]
edition = "2021"
description = "Makepad Doubao streaming ASR example"
license = "MIT OR Apache-2.0"

[dependencies]
makepad-widgets = { path = "../../widgets", version = "2.0.0" }
makepad-micro-serde = { path = "../../libs/micro_serde", version = "1.0.0" }
```

- [ ] **Step 2: Create `examples/doubao_asr/src/main.rs` (stub)**

```rust
pub use makepad_widgets;
mod doubao_asr;
use makepad_widgets::*;

app_main!(App);

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        crate::makepad_widgets::script_mod(vm);
        ScriptValue::None
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
```

- [ ] **Step 3: Create `examples/doubao_asr/src/doubao_asr.rs` (empty stub)**

```rust
// Doubao streaming ASR — state, protocol, and widget
```

- [ ] **Step 4: Add to workspace `Cargo.toml`**

In `/Users/alanpoon/Documents/rust/makepad/Cargo.toml`, find the `members` array and add `"examples/doubao_asr"` next to `"examples/speech_to_text"`.

- [ ] **Step 5: Verify compilation**

```bash
cd /Users/alanpoon/Documents/rust/makepad
cargo check -p makepad-example-doubao-asr
```

Expected: `Finished` with no errors (warnings about unused imports are OK at this stage).

- [ ] **Step 6: Commit**

```bash
git add examples/doubao_asr/ Cargo.toml
git commit -m "feat(doubao_asr): add project skeleton"
```

---

### Task 2: SessionState enum and DoubaoAsrState

**Files:**
- Modify: `examples/doubao_asr/src/doubao_asr.rs`

- [ ] **Step 1: Add state types to `doubao_asr.rs`**

```rust
use makepad_widgets::*;
use makepad_widgets::makepad_platform::makepad_network::{NetworkRuntime, WsSend, WsMessage};
use makepad_micro_serde::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub enum SessionState {
    Idle,
    Connecting,
    Streaming,
    Closing,
    Error(String),
}

impl Default for SessionState {
    fn default() -> Self { Self::Idle }
}

pub struct DoubaoAsrState {
    pub session:         Mutex<SessionState>,
    pub pending_samples: Mutex<Vec<f32>>,
    pub recent_samples:  Mutex<Vec<f32>>,
    pub confirmed_text:  Mutex<String>,
    pub interim_text:    Mutex<String>,
    pub is_recording:    AtomicBool,
}

impl DoubaoAsrState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            session:         Mutex::new(SessionState::Idle),
            pending_samples: Mutex::new(Vec::new()),
            recent_samples:  Mutex::new(Vec::new()),
            confirmed_text:  Mutex::new(String::new()),
            interim_text:    Mutex::new(String::new()),
            is_recording:    AtomicBool::new(false),
        })
    }

    pub fn start_recording(&self) {
        self.pending_samples.lock().unwrap().clear();
        self.recent_samples.lock().unwrap().clear();
        self.is_recording.store(true, Ordering::SeqCst);
    }

    pub fn stop_recording(&self) {
        self.is_recording.store(false, Ordering::SeqCst);
    }

    pub fn calculate_amplitude(&self) -> f32 {
        let recent = self.recent_samples.lock().unwrap();
        if recent.is_empty() { return 0.0; }
        let rms = (recent.iter().map(|s| s * s).sum::<f32>() / recent.len() as f32).sqrt();
        (rms * 30.0).min(1.0)
    }

    pub fn drain_pending_samples(&self) -> Vec<f32> {
        std::mem::take(&mut *self.pending_samples.lock().unwrap())
    }

    pub fn pending_sample_count(&self) -> usize {
        self.pending_samples.lock().unwrap().len()
    }

    pub fn set_session(&self, s: SessionState) {
        *self.session.lock().unwrap() = s;
    }

    pub fn is_busy(&self) -> bool {
        matches!(
            *self.session.lock().unwrap(),
            SessionState::Connecting | SessionState::Closing
        )
    }

    pub fn is_streaming(&self) -> bool {
        matches!(*self.session.lock().unwrap(), SessionState::Streaming)
    }

    pub fn take_error_msg(&self) -> Option<String> {
        if let SessionState::Error(msg) = &*self.session.lock().unwrap() {
            Some(msg.clone())
        } else {
            None
        }
    }
}
```

- [ ] **Step 2: Check compilation**

```bash
cargo check -p makepad-example-doubao-asr
```

Expected: no errors. The `NetworkRuntime` / `WsSend` / `WsMessage` imports may warn as unused — that's fine.

- [ ] **Step 3: Commit**

```bash
git add examples/doubao_asr/src/doubao_asr.rs
git commit -m "feat(doubao_asr): add SessionState and DoubaoAsrState"
```

---

### Task 3: Audio processing helpers

**Files:**
- Modify: `examples/doubao_asr/src/doubao_asr.rs`

- [ ] **Step 1: Add resampling, PCM encoding, and reqid helpers**

Append to `doubao_asr.rs`:

```rust
const ASR_SAMPLE_RATE: f64 = 16000.0;
const MAX_RECENT_SAMPLES: usize = 1600; // 100ms at 16kHz

pub fn resample_to_16k_mono(
    input: &makepad_widgets::makepad_platform::audio::AudioBuffer,
    from_rate: f64,
) -> Vec<f32> {
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

pub fn process_audio_input(
    state: &Arc<DoubaoAsrState>,
    info: makepad_widgets::makepad_platform::audio::AudioInfo,
    input_buffer: &makepad_widgets::makepad_platform::audio::AudioBuffer,
) {
    let resampled = resample_to_16k_mono(input_buffer, info.sample_rate);
    {
        let mut recent = state.recent_samples.lock().unwrap();
        recent.extend_from_slice(&resampled);
        let len = recent.len();
        if len > MAX_RECENT_SAMPLES {
            recent.drain(0..len - MAX_RECENT_SAMPLES);
        }
    }
    if state.is_recording.load(Ordering::SeqCst) {
        state.pending_samples.lock().unwrap().extend_from_slice(&resampled);
    }
}

pub fn f32_slice_to_pcm_i16_le(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        let i = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
        out.extend_from_slice(&i.to_le_bytes());
    }
    out
}

pub fn new_reqid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    format!("{:016x}{:08x}", d.as_secs(), d.subsec_nanos())
}
```

- [ ] **Step 2: Check compilation**

```bash
cargo check -p makepad-example-doubao-asr
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add examples/doubao_asr/src/doubao_asr.rs
git commit -m "feat(doubao_asr): add audio resampling and PCM helpers"
```

---

## Chunk 2: Protocol Encoding and Draw Structs

### Task 4: Binary frame building (client → server)

**Files:**
- Modify: `examples/doubao_asr/src/doubao_asr.rs`

The VolcEngine v2 frame has an 8-byte header:
- Byte 0: `0x11` (protocol version 1, header size 1 word)
- Byte 1: message type | flags
- Byte 2: serialization | compression
- Byte 3: `0x00` (reserved)
- Bytes 4–7: payload size as big-endian `u32`

- [ ] **Step 1: Add frame-building helpers**

Append to `doubao_asr.rs`:

```rust
fn build_frame(msg_type: u8, serialization: u8, payload: &[u8]) -> Vec<u8> {
    let size = payload.len() as u32;
    let mut frame = Vec::with_capacity(8 + payload.len());
    frame.push(0x11);
    frame.push(msg_type);
    frame.push(serialization);
    frame.push(0x00);
    frame.extend_from_slice(&size.to_be_bytes());
    frame.extend_from_slice(payload);
    frame
}

pub fn build_config_frame(app_id: &str, token: &str, reqid: &str) -> Vec<u8> {
    let json = format!(
        r#"{{"app":{{"appid":"{appid}","token":"{token}","cluster":"volcengine_streaming_common"}},"user":{{"uid":"makepad_doubao_asr"}},"audio":{{"format":"pcm","rate":16000,"encoding":"raw","bits":16,"channel":1,"codec":"raw"}},"request":{{"reqid":"{reqid}","sequence":1}}}}"#,
        appid = app_id,
        token = token,
        reqid = reqid,
    );
    // Byte 1: 0x10 = full client request, no flags
    // Byte 2: 0x10 = JSON serialization, no compression
    build_frame(0x10, 0x10, json.as_bytes())
}

pub fn build_audio_frame(pcm_bytes: &[u8], is_last: bool) -> Vec<u8> {
    // Byte 1: 0x20 = audio-only, 0x22 = audio-only + last-packet flag
    // Byte 2: 0x00 = raw (no JSON), no compression
    let msg_type = if is_last { 0x22 } else { 0x20 };
    build_frame(msg_type, 0x00, pcm_bytes)
}
```

- [ ] **Step 2: Check compilation**

```bash
cargo check -p makepad-example-doubao-asr
```

- [ ] **Step 3: Commit**

```bash
git add examples/doubao_asr/src/doubao_asr.rs
git commit -m "feat(doubao_asr): add VolcEngine v2 binary frame builders"
```

---

### Task 5: Response frame parsing (server → client)

**Files:**
- Modify: `examples/doubao_asr/src/doubao_asr.rs`

The server sends frames with the same 8-byte header. Skip the header and parse the UTF-8 JSON payload.

- [ ] **Step 1: Add JSON response types and parser**

Append to `doubao_asr.rs`:

```rust
#[derive(DeJson, Default)]
struct DoubaoResult {
    text: Option<String>,
    is_final: Option<bool>,
}

#[derive(DeJson, Default)]
struct DoubaoResponse {
    code: Option<i64>,
    message: Option<String>,
    result: Option<DoubaoResult>,
}

/// Parse a server binary frame. Returns None on malformed data.
pub fn parse_response_frame(data: &[u8]) -> Option<DoubaoResponse> {
    // Header is 8 bytes; payload starts at byte 8
    if data.len() < 8 { return None; }
    let payload = &data[8..];
    let json_str = std::str::from_utf8(payload).ok()?;
    DoubaoResponse::deserialize_json(json_str).ok()
}
```

- [ ] **Step 2: Check compilation**

```bash
cargo check -p makepad-example-doubao-asr
```

- [ ] **Step 3: Commit**

```bash
git add examples/doubao_asr/src/doubao_asr.rs
git commit -m "feat(doubao_asr): add response frame parser with DeJson"
```

---

### Task 6: DrawMicButton and DrawSpinner draw structs

**Files:**
- Modify: `examples/doubao_asr/src/doubao_asr.rs`

These are identical to the ones in `examples/speech_to_text/src/speech_input.rs`. The shaders are registered in the `script_mod!` block in `main.rs` (Task 10), not here.

- [ ] **Step 1: Add draw structs**

Append to `doubao_asr.rs`:

```rust
#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawMicButton {
    #[deref]
    pub draw_super: DrawQuad,
    #[live]
    pub is_recording: f32,
    #[live]
    pub amplitude: f32,
    #[live]
    pub accent_color: Vec4,
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawSpinner {
    #[deref]
    pub draw_super: DrawQuad,
    #[live]
    pub color: Vec4,
    #[live]
    pub time: f32,
}
```

- [ ] **Step 2: Check compilation**

```bash
cargo check -p makepad-example-doubao-asr
```

- [ ] **Step 3: Commit**

```bash
git add examples/doubao_asr/src/doubao_asr.rs
git commit -m "feat(doubao_asr): add DrawMicButton and DrawSpinner structs"
```

---

## Chunk 3: DoubaoAsrInput Widget

### Task 7: Widget struct and init

**Files:**
- Modify: `examples/doubao_asr/src/doubao_asr.rs`

- [ ] **Step 1: Add action enum and widget struct**

Append to `doubao_asr.rs`:

```rust
#[derive(Clone, Debug, Default)]
pub enum DoubaoAsrInputAction {
    #[default]
    None,
    RecordingStarted,
    RecordingStopped,
    InterimResult(String),
    FinalResult(String),
    Error(String),
}

#[derive(Script, ScriptHook, Widget)]
pub struct DoubaoAsrInput {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    #[find]
    #[redraw]
    #[live]
    text_input: WidgetRef,

    #[find]
    #[redraw]
    #[live]
    interim_label: WidgetRef,

    #[redraw]
    #[live]
    draw_mic: DrawMicButton,

    #[redraw]
    #[live]
    draw_spinner: DrawSpinner,

    #[redraw]
    #[live]
    draw_bg: DrawQuad,

    #[live(true)]
    #[visible]
    visible: bool,

    #[live]
    pub accent_color: Vec4,

    #[live(40.0)]
    pub mic_button_size: f64,

    #[rust]
    state: Option<Arc<DoubaoAsrState>>,

    #[rust]
    net_ref: Option<Arc<NetworkRuntime>>,

    #[rust]
    ws_id: LiveId,

    #[rust]
    current_amplitude: f32,

    #[rust]
    update_timer: Timer,

    #[rust]
    mic_area: Area,
}

impl DoubaoAsrInput {
    pub fn init(&mut self, cx: &mut Cx, state: Arc<DoubaoAsrState>, net: Arc<NetworkRuntime>, ws_id: LiveId) {
        self.state = Some(state);
        self.net_ref = Some(net);
        self.ws_id = ws_id;
        self.update_timer = cx.start_interval(0.033); // ~30fps
    }

    pub fn handle_action(&self, actions: &Actions) -> Option<DoubaoAsrInputAction> {
        actions
            .find_widget_action(self.widget_uid())
            .map(|a| a.cast::<DoubaoAsrInputAction>())
    }
}

impl WidgetMatchEvent for DoubaoAsrInput {
    fn handle_actions(&mut self, _cx: &mut Cx, _actions: &Actions, _scope: &mut Scope) {}
}
```

- [ ] **Step 2: Check compilation**

```bash
cargo check -p makepad-example-doubao-asr
```

- [ ] **Step 3: Commit**

```bash
git add examples/doubao_asr/src/doubao_asr.rs
git commit -m "feat(doubao_asr): add DoubaoAsrInput widget struct and init"
```

---

### Task 8: Widget drawing (`draw_walk`)

**Files:**
- Modify: `examples/doubao_asr/src/doubao_asr.rs`

- [ ] **Step 1: Implement `Widget::draw_walk` for `DoubaoAsrInput`**

Append to `doubao_asr.rs`:

```rust
impl Widget for DoubaoAsrInput {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.visible { return DrawStep::done(); }

        self.draw_mic.accent_color = self.accent_color;
        let button_walk = Walk::fixed(self.mic_button_size, self.mic_button_size);

        let is_busy = self.state.as_ref().map(|s| s.is_busy()).unwrap_or(false);

        if is_busy {
            self.draw_spinner.time = cx.time() as f32;
            self.draw_spinner.draw_walk(cx, button_walk);
        } else {
            self.draw_mic.draw_walk(cx, button_walk);
            self.mic_area = self.draw_mic.area();
        }

        self.text_input.draw_walk(cx, scope, walk);
        self.interim_label.draw_walk(cx, scope, Walk::default());

        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        // handled in Task 9
        let _ = (cx, event);
    }
}
```

- [ ] **Step 2: Check compilation**

```bash
cargo check -p makepad-example-doubao-asr
```

- [ ] **Step 3: Commit**

```bash
git add examples/doubao_asr/src/doubao_asr.rs
git commit -m "feat(doubao_asr): implement DoubaoAsrInput draw_walk"
```

---

### Task 9: Widget event handling and `update_state`

**Files:**
- Modify: `examples/doubao_asr/src/doubao_asr.rs`

This is the core of the widget. Replace the `handle_event` stub from Task 8 with the full implementation, and add `update_state`.

- [ ] **Step 1: Replace `handle_event` body and add `update_state`**

Replace the body of `fn handle_event` inside `impl Widget for DoubaoAsrInput`:

```rust
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if !self.visible { return; }

        if let Event::Timer(te) = event {
            if self.update_timer.is_timer(te).is_some() {
                // Drain audio and update UI every ~33ms, directly from the timer.
                // Do NOT emit widget_action here — Actions are only dispatched when
                // cx.new_actions is non-empty, so we can't rely on handle_actions for timing.
                let ws_id = self.ws_id;
                self.drain_and_send(cx, ws_id);
                self.update_ui(cx);
            }
        }

        let is_busy = self.state.as_ref().map(|s| s.is_busy()).unwrap_or(false);
        if !is_busy {
            if let Hit::FingerDown(_) = event.hits(cx, self.mic_area) {
                let is_recording = self.state.as_ref()
                    .map(|s| s.is_recording.load(Ordering::SeqCst))
                    .unwrap_or(false);
                if is_recording {
                    cx.widget_action(self.widget_uid(), DoubaoAsrInputAction::RecordingStopped);
                } else {
                    cx.widget_action(self.widget_uid(), DoubaoAsrInputAction::RecordingStarted);
                }
            }
        }
    }
```

Then add the helper methods as a second `impl DoubaoAsrInput` block:

```rust
impl DoubaoAsrInput {
    /// Drain pending PCM samples and send to WebSocket (called from timer).
    fn drain_and_send(&mut self, _cx: &mut Cx, ws_id: LiveId) {
        let state = match &self.state { Some(s) => s.clone(), None => return };
        let net   = match &self.net_ref { Some(n) => n.clone(), None => return };
        if state.is_streaming() {
            let samples = state.drain_pending_samples();
            if !samples.is_empty() {
                let pcm = f32_slice_to_pcm_i16_le(&samples);
                let frame = build_audio_frame(&pcm, false);
                let _ = net.ws_send(ws_id, WsSend::Binary(frame));
            }
        }
    }

    /// Refresh visual state from shared state (called from timer).
    fn update_ui(&mut self, cx: &mut Cx) {
        let state = match &self.state { Some(s) => s.clone(), None => return };

        let amplitude = state.calculate_amplitude();
        self.current_amplitude = self.current_amplitude * 0.7 + amplitude * 0.3;
        self.draw_mic.amplitude = self.current_amplitude;
        self.draw_mic.is_recording = if state.is_recording.load(Ordering::SeqCst) { 1.0 } else { 0.0 };

        let interim = state.interim_text.lock().unwrap().clone();
        // Use borrow_mut + set_text (Label has no set_text_and_redraw method)
        if let Some(mut label) = self.interim_label.borrow_mut::<Label>() {
            label.set_text(cx, &interim);
        }

        self.redraw(cx);
    }

    /// Public method called by App to set ws_id after init (used in handle_startup).
    pub fn set_ws_id(&mut self, ws_id: LiveId) {
        self.ws_id = ws_id;
    }

    /// Public update hook for App to call if needed; audio drain is now in the timer directly.
    pub fn update_state(&mut self, cx: &mut Cx, ws_id: LiveId) {
        self.drain_and_send(cx, ws_id);
        self.update_ui(cx);
    }
}
```

- [ ] **Step 2: Check compilation**

```bash
cargo check -p makepad-example-doubao-asr
```

Fix any type errors — common ones: `interim_label.label(cx, ids!(...))` — check the correct widget accessor for `Label` (it may be `.label(cx, ids!(interim_label))` or direct `.set_text(cx, &text)`).

The correct Makepad pattern for a found widget ref is:
```rust
self.interim_label.apply_over(cx, live!{ text: (interim) });
// or use the Label widget accessor:
if let Some(mut label) = self.interim_label.borrow_mut::<Label>() {
    label.set_text_and_redraw(cx, &interim);
}
```

Use whichever compiles. Check `examples/speech_to_text/src/speech_input.rs` for the `set_text` pattern on `WidgetRef`.

- [ ] **Step 3: Commit**

```bash
git add examples/doubao_asr/src/doubao_asr.rs
git commit -m "feat(doubao_asr): implement widget event handling and update_state"
```

---

## Chunk 4: App (main.rs) — UI Script, Event Handlers, and Wiring

### Task 10: App struct and UI script block

**Files:**
- Modify: `examples/doubao_asr/src/main.rs`

- [ ] **Step 1: Replace `main.rs` stub with full implementation**

```rust
pub use makepad_widgets;

mod doubao_asr;

use makepad_widgets::makepad_draw::CxMediaApi;
use makepad_widgets::*;
use makepad_widgets::makepad_platform::makepad_network::NetworkRuntime;

use doubao_asr::{
    DrawMicButton, DrawSpinner, DoubaoAsrInput, DoubaoAsrInputAction,
    DoubaoAsrState, process_audio_input,
};

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*

    // Mic button shader (same as speech_to_text, accent color #00AAFF)
    set_type_default() do #(DrawMicButton::script_shader(vm)){
        ..mod.draw.DrawQuad
        is_recording: 0.0
        amplitude: 0.0
        accent_color: #00AAFF

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
        color: #00AAFF
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

    // Register DoubaoAsrInput widget
    mod.widgets.DoubaoAsrInputBase = #(DoubaoAsrInput::register_widget(vm))
    mod.widgets.DoubaoAsrInput = set_type_default() do mod.widgets.DoubaoAsrInputBase {
        width: Fill
        height: Fit
        flow: Down
        spacing: 6

        accent_color: #00AAFF
        mic_button_size: 40.0

        draw_spinner.color: #00AAFF

        text_input := TextInput {
            width: Fill
            height: 50
            empty_text: "Type or speak Chinese..."
            draw_bg.color: #FFFFFF
            draw_bg.border_color: #00AAFF
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
                window.title: "Doubao ASR — Speech to Text"
                window.inner_size: vec2(700, 300)
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

                        asr_input := mod.widgets.DoubaoAsrInput{}
                    }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,

    #[rust]
    ws_id: LiveId,

    #[rust]
    asr_state: Option<Arc<DoubaoAsrState>>,

    #[rust]
    app_id: String,

    #[rust]
    access_token: String,

    #[rust]
    audio_initialized: bool,
}
```

> **Note on `ws_id` initialization:** Initialize it in `handle_startup` with `self.ws_id = live_id!(doubao_asr_socket);`. The `live_id!` macro computes a compile-time hash from the string — always non-zero, never collides with the internal studio socket (which is `LiveId(0)`).

- [ ] **Step 2: Check compilation**

```bash
cargo check -p makepad-example-doubao-asr
```

This step will have errors because `App`'s trait impls are missing — that's expected. Fix any import errors or struct definition issues only.

- [ ] **Step 3: Commit**

```bash
git add examples/doubao_asr/src/main.rs
git commit -m "feat(doubao_asr): add App struct and UI script block"
```

---

### Task 11: App event handlers

**Files:**
- Modify: `examples/doubao_asr/src/main.rs`

- [ ] **Step 1: Add `MatchEvent`, `AppMain` impls, and helper methods**

Append to `main.rs`:

```rust
use std::sync::Arc;
use makepad_widgets::makepad_platform::makepad_network::{WsSend, HttpRequest, HttpMethod};

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        self.ws_id = live_id!(doubao_asr_socket);
        self.app_id     = std::env::var("DOUBAO_APP_ID").unwrap_or_default();
        self.access_token = std::env::var("DOUBAO_ACCESS_TOKEN").unwrap_or_default();

        let state = DoubaoAsrState::new();
        self.asr_state = Some(state.clone());

        if let Some(mut widget) = self.ui.widget(cx, ids!(asr_input)).borrow_mut::<DoubaoAsrInput>() {
            widget.init(cx, state, cx.net.clone(), self.ws_id);
        }

        if self.app_id.is_empty() || self.access_token.is_empty() {
            self.ui.label(cx, ids!(status_label)).set_text(
                cx, "Set DOUBAO_APP_ID and DOUBAO_ACCESS_TOKEN env vars"
            );
        }

        cx.use_audio_inputs(&[]);
    }

    fn handle_audio_devices(&mut self, cx: &mut Cx, devices: &AudioDevicesEvent) {
        cx.use_audio_inputs(&devices.default_input());
        self.audio_initialized = true;
        self.wire_audio_input(cx);
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        // Audio draining happens in the widget's 30fps timer (handle_event), not here.
        // handle_actions is only called when actions exist (user interaction), so it
        // cannot reliably drive the audio drain loop.

        // Handle mic button actions only.
        if let Some(asr_widget) = self.ui.widget(cx, ids!(asr_input)).borrow::<DoubaoAsrInput>() {
            if let Some(action) = asr_widget.handle_action(actions) {
                match action {
                    DoubaoAsrInputAction::RecordingStarted => {
                        self.on_recording_started(cx);
                    }
                    DoubaoAsrInputAction::RecordingStopped => {
                        self.on_recording_stopped(cx);
                    }
                    _ => {}
                }
            }
        }
    }
}

impl App {
    fn on_recording_started(&mut self, cx: &mut Cx) {
        if self.app_id.is_empty() || self.access_token.is_empty() { return; }

        let state = match &self.asr_state { Some(s) => s.clone(), None => return };

        // Guard: if a socket is still open, close it first
        let _ = cx.net.ws_close(self.ws_id);

        state.start_recording();
        state.set_session(DoubaoAsrState::connecting_state());

        let mut request = HttpRequest::new(
            "wss://openspeech.bytedance.com/api/v2/asr".to_string(),
            HttpMethod::GET,
        );
        request.set_header("X-Api-App-Key".to_string(), self.app_id.clone());
        request.set_header("X-Api-Access-Key".to_string(), self.access_token.clone());

        if let Err(e) = cx.net.ws_open(self.ws_id, request) {
            state.set_session(SessionState::Error(format!("ws_open failed: {e}")));
        }

        self.ui.label(cx, ids!(status_label)).set_text(cx, "Connecting…");
    }

    fn on_recording_stopped(&mut self, cx: &mut Cx) {
        let state = match &self.asr_state { Some(s) => s.clone(), None => return };
        state.stop_recording();

        // Send EOS (last audio frame, empty payload)
        use doubao_asr::build_audio_frame;
        let eos = build_audio_frame(&[], true);
        let _ = cx.net.ws_send(self.ws_id, WsSend::Binary(eos));

        state.set_session(SessionState::Closing);
        self.ui.label(cx, ids!(status_label)).set_text(cx, "Waiting for final result…");
    }

    fn wire_audio_input(&mut self, cx: &mut Cx) {
        let state = match &self.asr_state { Some(s) => s.clone(), None => return };
        cx.audio_input(0, move |info, buf| {
            process_audio_input(&state, info, buf);
        });
    }

    fn handle_network_responses(&mut self, cx: &mut Cx, responses: &[makepad_widgets::makepad_platform::makepad_network::NetworkResponse]) {
        use makepad_widgets::makepad_platform::makepad_network::NetworkResponse;

        for response in responses {
            match response {
                NetworkResponse::WsOpened { socket_id } if *socket_id == self.ws_id => {
                    self.on_ws_opened(cx);
                }
                NetworkResponse::WsMessage { socket_id, message: WsMessage::Binary(data) }
                    if *socket_id == self.ws_id =>
                {
                    self.on_ws_message(cx, data);
                }
                NetworkResponse::WsError { socket_id, message }
                    if *socket_id == self.ws_id =>
                {
                    self.on_ws_error(cx, message.clone());
                }
                NetworkResponse::WsClosed { socket_id } if *socket_id == self.ws_id => {
                    self.on_ws_closed(cx);
                }
                _ => {}
            }
        }
    }

    fn on_ws_opened(&mut self, cx: &mut Cx) {
        let state = match &self.asr_state { Some(s) => s.clone(), None => return };
        use doubao_asr::build_config_frame;
        let reqid = doubao_asr::new_reqid();
        let frame = build_config_frame(&self.app_id, &self.access_token, &reqid);
        let _ = cx.net.ws_send(self.ws_id, WsSend::Binary(frame));
        state.set_session(SessionState::Streaming);
        self.ui.label(cx, ids!(status_label)).set_text(cx, "Recording — click mic to stop");
    }

    fn on_ws_message(&mut self, cx: &mut Cx, data: &[u8]) {
        let state = match &self.asr_state { Some(s) => s.clone(), None => return };
        use doubao_asr::parse_response_frame;
        let Some(resp) = parse_response_frame(data) else {
            log!("doubao_asr: failed to parse response frame");
            return;
        };

        let code = resp.code.unwrap_or(1000);
        if code != 1000 {
            let msg = resp.message.unwrap_or_else(|| format!("ASR error code {code}"));
            state.set_session(SessionState::Error(msg.clone()));
            self.ui.label(cx, ids!(status_label)).set_text(cx, &format!("Error: {msg}"));
            return;
        }

        if let Some(result) = resp.result {
            let text = result.text.unwrap_or_default();
            let is_final = result.is_final.unwrap_or(false);

            if is_final {
                // Append to TextInput (preserve any manually typed text)
                let existing = self.ui.text_input(cx, ids!(asr_input.text_input)).text();
                let new_text = format!("{}{}", existing, text);
                self.ui.text_input(cx, ids!(asr_input.text_input)).set_text(cx, &new_text);
                *state.interim_text.lock().unwrap() = String::new();
                *state.confirmed_text.lock().unwrap() = new_text;

                // If in Closing state, transition to Idle
                if matches!(*state.session.lock().unwrap(), SessionState::Closing) {
                    state.set_session(SessionState::Idle);
                    state.is_recording.store(false, Ordering::SeqCst);
                    self.ui.label(cx, ids!(status_label)).set_text(cx, "Ready — click microphone to start");
                }
            } else {
                *state.interim_text.lock().unwrap() = text;
            }
        }
    }

    fn on_ws_error(&mut self, cx: &mut Cx, message: String) {
        let state = match &self.asr_state { Some(s) => s.clone(), None => return };
        state.stop_recording();
        state.pending_samples.lock().unwrap().clear();
        state.set_session(SessionState::Error(message.clone()));
        self.ui.label(cx, ids!(status_label)).set_text(cx, &format!("Error: {message}"));
    }

    fn on_ws_closed(&mut self, cx: &mut Cx) {
        let state = match &self.asr_state { Some(s) => s.clone(), None => return };
        let session = std::mem::replace(&mut *state.session.lock().unwrap(), SessionState::Idle);
        match session {
            SessionState::Closing => {
                // Normal end — already transitioned to Idle
                self.ui.label(cx, ids!(status_label)).set_text(cx, "Ready — click microphone to start");
            }
            SessionState::Streaming => {
                let msg = "Session closed unexpectedly".to_string();
                *state.session.lock().unwrap() = SessionState::Error(msg.clone());
                state.pending_samples.lock().unwrap().clear();
                state.stop_recording();
                self.ui.label(cx, ids!(status_label)).set_text(cx, &format!("Error: {msg}"));
            }
            _ => {
                *state.session.lock().unwrap() = SessionState::Idle;
            }
        }
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

        if let Event::AudioDevices(_) = event {
            if self.audio_initialized {
                self.wire_audio_input(cx);
            }
        }

        if let Event::NetworkResponses(responses) = event {
            let responses = responses.clone();
            self.handle_network_responses(cx, &responses);
        }
    }
}
```

> **`DoubaoAsrState::connecting_state()`** — this helper doesn't exist yet. Add it to `DoubaoAsrState`:
> ```rust
> pub fn connecting_state() -> SessionState { SessionState::Connecting }
> ```
> Or simply inline: `state.set_session(SessionState::Connecting)` (requires importing `SessionState` in `main.rs` via `use doubao_asr::SessionState`).

- [ ] **Step 2: Add `SessionState` to the public exports of `doubao_asr.rs`**

Ensure `SessionState`, `DoubaoAsrState`, `new_reqid`, `build_config_frame`, `build_audio_frame`, `parse_response_frame`, and `process_audio_input` are all `pub`. Add `use std::sync::atomic::Ordering;` to `main.rs`.

- [ ] **Step 3: Check compilation**

```bash
cargo check -p makepad-example-doubao-asr
```

Fix type errors iteratively. Common issues:
- `socket_id` type: it's `LiveId` — compare with `== self.ws_id` (both are `LiveId`)
- `WsMessage` name conflict: the enum and field are both named `WsMessage` — use the fully-qualified variant `makepad_network::WsMessage::Binary(data)` or alias the type at the top of the match arm
- Label `set_text` is on `WidgetRef`, call as `self.ui.label(cx, ids!(status_label)).set_text(cx, &text)` — if this accessor doesn't exist, use `self.ui.widget(cx, ids!(status_label)).set_text(cx, &text)` instead

- [ ] **Step 4: Commit**

```bash
git add examples/doubao_asr/src/main.rs examples/doubao_asr/src/doubao_asr.rs
git commit -m "feat(doubao_asr): implement App event handlers and NetworkResponse routing"
```

---

### Task 12: Integration check and manual testing

> `pending_sample_count()` was already added to `DoubaoAsrState` in Task 2.
> The short-recording guard in `on_recording_stopped` (Task 11) uses it directly.

- [ ] **Step 1: Final compilation check**

```bash
cargo check -p makepad-example-doubao-asr
```

Expected: clean compile with no errors.

- [ ] **Step 4: Run and test manually**

```bash
# Set credentials (replace with real values for actual test)
export DOUBAO_APP_ID=your_app_id
export DOUBAO_ACCESS_TOKEN=your_access_token
cargo run -p makepad-example-doubao-asr
```

Walk through the test scenarios from spec section 12:

1. **Missing credentials:** Run without env vars — status shows credential instructions
2. **Short recording:** Click mic, immediately click again — "Recording too short"
3. **Happy path (requires real credentials):** Click mic → speak Chinese → interim gray text → click mic → final text appears

- [ ] **Step 5: Commit**

```bash
git add examples/doubao_asr/src/doubao_asr.rs examples/doubao_asr/src/main.rs
git commit -m "feat(doubao_asr): add short-recording guard, complete implementation"
```

---

## Summary

| Chunk | Tasks | Outcome |
|-------|-------|---------|
| 1 | 1–3 | Skeleton compiles; state + audio utilities in place |
| 2 | 4–6 | Protocol encoding/decoding and draw structs ready |
| 3 | 7–9 | `DoubaoAsrInput` widget fully functional |
| 4 | 10–12 | App event handlers, UI, and wiring complete; manual tests pass |

**Run the final binary:**
```bash
export DOUBAO_APP_ID=...
export DOUBAO_ACCESS_TOKEN=...
cargo run -p makepad-example-doubao-asr
```
