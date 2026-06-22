# Doubao ASR Makepad Example — Design Spec

**Date:** 2026-06-17  
**Status:** Approved  
**Location:** `examples/doubao_asr/`

---

## 1. Purpose

Create a new Makepad desktop example (`doubao_asr`) that mirrors `examples/speech_to_text` in look and feel but replaces the local Whisper model with Doubao/VolcEngine's **streaming WebSocket ASR** API (`wss://openspeech.bytedance.com/api/v2/asr`). The example demonstrates real-time Chinese (zh-CN) speech recognition with interim results shown live as the user speaks.

---

## 2. Scope

**In scope:**
- Makepad desktop app (macOS primary target, same platform support as `speech_to_text`)
- Streaming WebSocket ASR via VolcEngine v2 binary frame protocol
- Audio capture via Makepad's existing audio API (same `cx.audio_input()` pattern)
- Real-time interim text display (gray) + final confirmed text in TextInput
- Credentials via env vars `DOUBAO_APP_ID` and `DOUBAO_ACCESS_TOKEN`
- Chinese (zh-CN) language, `volcengine_streaming_common` cluster

**Out of scope:**
- Language selector UI
- HTTP batch/offline ASR fallback
- Token refresh or OAuth flow
- Saving transcription to file
- Mobile/WASM targets

---

## 3. File Structure

```
examples/doubao_asr/
├── Cargo.toml
└── src/
    ├── main.rs          # App, UI script, event dispatch
    └── doubao_asr.rs    # Widget, state, protocol encoding/decoding
```

**Dependencies** (Cargo.toml):
```toml
makepad-widgets = { path = "../../widgets", version = "2.0.0" }
```

No new external crates. The WebSocket uses `cx.net.ws_open()` and `cx.net.ws_send()` from `NetworkRuntime` (the `pub net: Arc<NetworkRuntime>` field on `Cx`). On macOS the network backend uses `NSURLSession` which supports TLS (`wss://`) natively.

**Import path for `WsSend` / `WsMessage`:**
```rust
use makepad_widgets::makepad_platform::makepad_network::{WsSend, WsMessage};
```

The `NetworkResponse` variants (`WsOpened`, `WsMessage`, `WsClosed`, `WsError`) arrive automatically via `Event::NetworkResponses` — the Makepad event loop calls `dispatch_network_runtime_events()` which polls `cx.net.try_recv()` on every frame.

---

## 4. State Machine

```
Idle ──(mic click, creds OK)──► Connecting
Connecting ──(WsOpened)──────► Streaming  (send config frame)
Streaming ──(mic click)──────► Closing    (send EOS audio frame)
Closing ──(WsMessage final)──► Idle       (clear interim, keep final text)
Any ──(WsError / WsClosed)──► Error ──(timer)──► Idle
```

Mic button is disabled during `Connecting` and `Closing`.

---

## 5. Session State Enum

```rust
pub enum SessionState {
    Idle,
    Connecting,   // ws_open called, waiting for WsOpened
    Streaming,    // WsOpened received, config sent, audio flowing
    Closing,      // EOS frame sent, waiting for final WsMessage
    Error(String),
}
```

Auto-transition: `Error` → `Idle` on the next timer tick (one ~33ms cycle is sufficient to ensure the status label has been redrawn before clearing the error).

## 5b. Shared State (`Arc<DoubaoAsrState>`)

```rust
pub struct DoubaoAsrState {
    pub session:          Mutex<SessionState>,   // enum above
    pub pending_samples:  Mutex<Vec<f32>>,       // PCM waiting to be sent (drained on timer)
    pub recent_samples:   Mutex<Vec<f32>>,       // last 100ms for amplitude
    pub confirmed_text:   Mutex<String>,          // final, committed text
    pub interim_text:     Mutex<String>,          // latest partial result
    pub error:            Mutex<Option<String>>,  // last error message
    pub is_recording:     AtomicBool,
}
```

The audio callback writes to `pending_samples` and `recent_samples`. The widget's `update_state()` method (called from its 30fps timer) drains `pending_samples`, encodes it as PCM i16, and sends it via `cx.net.ws_send()`. The same timer tick also reads `confirmed_text`, `interim_text`, and `error` to update the UI labels.

On any transition to `Idle` or `Error`, `pending_samples` is cleared to prevent stale audio being sent on the next session.

---

## 6. Doubao Binary Frame Protocol (VolcEngine v2)

### Frame layout (client → server)

```
Byte 0:  (protocol_version << 4) | header_size_words  = 0x11
Byte 1:  (message_type    << 4) | type_flags          = see below
Byte 2:  (serialization   << 4) | compression         = see below
Byte 3:  reserved                                      = 0x00
Bytes 4-7: payload_size (big-endian u32)
Bytes 8+:  payload
```

| Frame | Byte 1 | Byte 2 | Payload |
|-------|--------|--------|---------|
| Config (first) | `0x10` (full client request) | `0x10` (JSON, no compress) | Config JSON (UTF-8) |
| Audio chunk    | `0x20` (audio only)          | `0x00` (raw, no compress)  | PCM i16-LE bytes   |
| Last audio     | `0x22` (audio-only + last)   | `0x00`                     | PCM i16-LE bytes (may be empty) |

### Config JSON template

```json
{
  "app": { "appid": "{DOUBAO_APP_ID}", "token": "{DOUBAO_ACCESS_TOKEN}",
           "cluster": "volcengine_streaming_common" },
  "user": { "uid": "makepad_doubao_asr" },
  "audio": { "format": "pcm", "rate": 16000, "encoding": "raw",
              "bits": 16, "channel": 1, "codec": "raw" },
  "request": { "reqid": "{reqid}", "sequence": 1 }
}
```

**`reqid` generation** (no `uuid` crate available): use a simple timestamp-based hex string:
```rust
fn new_reqid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    format!("{:016x}{:08x}", d.as_secs(), d.subsec_nanos())
}
```

**`sequence` field**: only present in the config JSON frame (the initial full-client-request). Audio-only binary frames (`0x20` / `0x22`) carry no JSON payload and thus no `sequence` field. The server handles sequencing internally from the frame headers.

### Server response

Response frames use the same 8-byte header. Skip the header and parse the payload as UTF-8 JSON:

```json
{
  "reqid": "...",
  "code": 1000,
  "sequence": -1,
  "message": "success",
  "result": { "text": "...", "is_final": true }
}
```

- `code != 1000` → treat as error; show `message` field in status label
- `is_final == true` → append `result.text` to `confirmed_text`; clear `interim_text`
- `is_final == false` → update `interim_text`

---

## 7. Audio Processing

Port `process_audio_input()` from `speech_to_text` (same logic, different struct target):
- Resample any input sample rate → 16 kHz mono using linear interpolation
- Append to `pending_samples` only when `is_recording == true`
- Always update `recent_samples` (last 100ms for amplitude visualization, capped at 1600 samples)

The field is named `pending_samples` (not `accumulated_samples` as in `speech_to_text`) to reflect that samples are streamed continuously rather than held until end-of-recording.

**PCM encoding** (done when draining on the timer, not in the audio callback):
```rust
fn f32_slice_to_pcm_i16_le(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        let i = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
        out.extend_from_slice(&i.to_le_bytes());
    }
    out
}
```

**Audio chunk size**: drain all samples in `pending_samples` on each timer tick (~33ms at 30fps = ~528 samples at 16kHz). This produces chunks of variable size. VolcEngine's v2 streaming API accepts variable-size audio chunks; there is no enforced minimum. Empty drains (during `Connecting` or `Idle`) produce no `ws_send` call.

---

## 8. Widget: `DoubaoAsrInput`

### Fields (same pattern as `SpeechInput`)

```rust
pub struct DoubaoAsrInput {
    #[uid]    uid: WidgetUid,
    #[source] source: ScriptObjectRef,
    #[walk]   walk: Walk,
    #[layout] layout: Layout,
    #[find] #[redraw] #[live] text_input: WidgetRef,    // confirmed text
    #[find] #[redraw] #[live] interim_label: WidgetRef, // interim text (gray)
    #[redraw] #[live] draw_mic: DrawMicButton,
    #[redraw] #[live] draw_spinner: DrawSpinner,
    #[redraw] #[live] draw_bg: DrawQuad,
    #[live(true)] #[visible] visible: bool,
    #[live] accent_color: Vec4,
    #[live(40.0)] mic_button_size: f64,
    #[rust] state: Option<Arc<DoubaoAsrState>>,
    #[rust] net_ref: Option<Arc<NetworkRuntime>>,  // clone of cx.net, set via init()
    #[rust] current_amplitude: f32,
    #[rust] update_timer: Timer,
    #[rust] mic_area: Area,
}
```

`is_busy` is derived on-the-fly from `state.session` (check for `Connecting | Closing`); it is not a separate stored field.

### Timer ownership and NetworkRuntime access

The widget owns `update_timer` (started in `init(cx)`). The App calls `widget.init(cx)` in `handle_startup`, which also stores `cx.net.clone()` in `net_ref`. The widget's `update_state(cx)` drains `pending_samples` and sends audio via `self.net_ref.as_ref().unwrap().ws_send(ws_id, WsSend::Binary(pcm_bytes))`.

The `ws_id` must also be available to `update_state` — it is passed as a parameter:
```rust
fn update_state(&mut self, cx: &mut Cx, ws_id: LiveId) { ... }
```

The App calls `widget.update_state(cx, self.ws_id)` from its `handle_event` timer arm (after routing via `self.match_event(cx, event)`).

### Actions

```rust
pub enum DoubaoAsrInputAction {
    None, RecordingStarted, RecordingStopped,
    InterimResult(String), FinalResult(String), Error(String),
}
```

### `draw_walk`: same as `SpeechInput` — spinner while busy, mic button otherwise

### `handle_event`: 
- Timer → call `update_state(cx)` (poll shared state, redraw)
- `Hit::FingerDown` on `mic_area` → emit `RecordingStarted` or `RecordingStopped` action
- App's `handle_actions` responds by opening/closing WebSocket

---

## 9. App (`main.rs`)

### Rust struct

```rust
#[derive(Script, ScriptHook)]
pub struct App {
    #[live] ui: WidgetRef,
    #[rust] ws_id: LiveId,          // MUST be live_id!(doubao_asr_socket) — non-zero
    #[rust] asr_state: Option<Arc<DoubaoAsrState>>,
    #[rust] app_id: String,
    #[rust] access_token: String,
    #[rust] audio_initialized: bool,
}
```

**Socket ID**: always use `live_id!(doubao_asr_socket)` (evaluated at compile time to a non-zero hash). The internal studio WebSocket uses socket ID `0`; using `LiveId(0)` or `LiveId::empty()` will silently swallow all ASR responses.

### Event handling

| Event | Action |
|-------|--------|
| `handle_startup` | Read env vars; init `DoubaoAsrState` + pass `Arc<NetworkRuntime>` to widget; start audio |
| `handle_audio_devices` | `cx.use_audio_inputs(devices.default_input())`; wire `cx.audio_input()` callback |
| `DoubaoAsrInputAction::RecordingStarted` | Build `HttpRequest` for `wss://...`; set auth headers; call `cx.net.ws_open(ws_id, request)` |
| `DoubaoAsrInputAction::RecordingStopped` | Send EOS binary frame (type `0x22`, empty payload) via `cx.net.ws_send()`; after this, no more audio frames are sent |
| `Event::NetworkResponses` with `WsOpened {socket_id}` matching `ws_id` | Send config JSON binary frame via `cx.net.ws_send()`; set session to `Streaming` |
| `Event::NetworkResponses` with `WsMessage {socket_id, message: WsMessage::Binary(data)}` | Parse response; update `confirmed_text` / `interim_text` |
| `Event::NetworkResponses` with `WsError` | Set `error` in state; set session to `Error` |
| `Event::NetworkResponses` with `WsClosed` in `Streaming` state | Treat as error ("Session closed unexpectedly"); set session to `Error` |
| `Event::NetworkResponses` with `WsClosed` in `Closing` state | Set session to `Idle` (normal end) |

The widget's 30fps timer handles UI refresh and audio draining (not a separate App-level timer).

### WebSocket authentication headers

```
X-Api-App-Key: {DOUBAO_APP_ID}
X-Api-Access-Key: {DOUBAO_ACCESS_TOKEN}
```

Set via `request.set_header()` before `cx.net.ws_open()`. The credentials are also embedded in the config JSON payload (`app.appid` and `app.token`). Both are sent to satisfy different VolcEngine endpoint configurations.

### Accumulating text across sessions

When appending a final result to the TextInput, always read the current TextInput text first and append to it — do not overwrite. This preserves any text the user has manually typed or edited between sessions:
```rust
let existing = self.ui.text_input(cx, ids!(asr_input.text_input)).text();
let new_text = format!("{}{}", existing, final_text);
self.ui.text_input(cx, ids!(asr_input.text_input)).set_text(cx, &new_text);
```

---

## 10. UI Layout (Script Block)

```
Window (700×300, title: "Doubao ASR — Speech to Text")
└── View (Fill×Fill, flow: Down, padding: 30, bg: #5A5A5A)
    ├── Label status_label ("Ready — click microphone to start", #CCCCCC)
    └── DoubaoAsrInput
        ├── [mic button / spinner]
        ├── TextInput text_input (Fill×50, border #00AAFF radius 25)
        └── Label interim_label ("", #888888, font_size 12)
```

Accent color: `#00AAFF` (blue, to visually distinguish from the orange Whisper example).

`interim_label` uses `height: Fit` so it expands for multi-line Chinese ASR output without clipping.

---

## 11. Error Handling

| Scenario | Behavior |
|---|---|
| Missing env vars at startup | Status: "Set DOUBAO_APP_ID and DOUBAO_ACCESS_TOKEN"; mic button click is a no-op |
| `WsError` from Makepad | Session → `Error(message)`; status shows message; next timer tick → `Idle`; mic re-enabled |
| `code != 1000` in server JSON response | Extract `message` field; session → `Error(message)`; mic re-enabled on next tick |
| Recording < 1600 samples (< 100ms audio) | Skip `ws_open`; status: "Recording too short" |
| `WsClosed` during `Streaming` state | Session → `Error("Session closed unexpectedly")`; clear pending audio |
| `WsClosed` during `Closing` state | Session → `Idle`; normal end |
| Parse failure on server binary frame | `crate::log!("doubao_asr: failed to parse response frame")`; ignore frame; stay in current state |
| Server session timeout (~60s inactivity) | Server sends `WsClosed`; handled as per `WsClosed` row above |
| Multiple mic clicks during `Connecting` | Mic button is disabled (spinner shown); click is ignored |
| `ws_open` called while previous socket still open | Call `cx.net.ws_close(ws_id)` first, then `cx.net.ws_open()` |

---

## 12. Testing Plan (Manual)

1. **Happy path:** Set env vars → run → mic click → speak Chinese → see interim gray text → click mic → final text in TextInput, interim clears, status "Ready"
2. **Missing credentials:** Run without env vars → mic button is dimmed; status shows credential instructions
3. **Network error:** Start recording → kill network → `WsError` appears in status, mic re-enables
4. **Very short recording:** Click mic, immediately click again → "Recording too short" message
5. **Multiple utterances:** Click mic → speak → click → speak → click → text accumulates in TextInput across sessions
