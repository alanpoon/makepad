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

No new external crates. The WebSocket uses Makepad's built-in `cx.web_socket_open()` / `cx.web_socket_send_binary()` + `NSURLSession` on macOS (TLS supported).

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

## 5. Shared State (`Arc<DoubaoAsrState>`)

```rust
pub struct DoubaoAsrState {
    pub session:          Mutex<SessionState>,   // enum above
    pub pending_samples:  Mutex<Vec<f32>>,       // PCM waiting to be sent
    pub recent_samples:   Mutex<Vec<f32>>,       // last 100ms for amplitude
    pub confirmed_text:   Mutex<String>,          // final, committed text
    pub interim_text:     Mutex<String>,          // latest partial result
    pub error:            Mutex<Option<String>>,  // last error message
    pub is_recording:     AtomicBool,
}
```

The audio callback writes to `pending_samples` and `recent_samples`. The main event handler reads/drains `pending_samples` on every timer tick and sends it over WebSocket. The UI timer reads `confirmed_text`, `interim_text`, and `error` to update labels.

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
  "request": { "reqid": "{uuid}", "sequence": 1 }
}
```

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

Reuse `process_audio_input()` from `speech_to_text`:
- Resample any input sample rate → 16 kHz mono using linear interpolation
- Append to `pending_samples` only when `is_recording == true`
- Always update `recent_samples` (last 100ms) for amplitude visualization

PCM encoding: convert `f32 [-1,1]` → `i16` by `(sample * 32767.0).clamp(-32768.0, 32767.0) as i16`, write as little-endian bytes.

---

## 8. Widget: `DoubaoAsrInput`

### Fields (same pattern as `SpeechInput`)

```rust
pub struct DoubaoAsrInput {
    uid: WidgetUid, source: ScriptObjectRef,
    walk: Walk, layout: Layout,
    #[find] text_input: WidgetRef,         // confirmed text
    #[find] interim_label: WidgetRef,      // interim text (gray)
    draw_mic: DrawMicButton,               // same shader as speech_to_text
    draw_spinner: DrawSpinner,             // same shader
    draw_bg: DrawQuad,
    visible: bool,
    accent_color: Vec4,
    mic_button_size: f64,
    state: Option<Arc<DoubaoAsrState>>,
    current_amplitude: f32,
    is_busy: bool,                         // true during Connecting/Closing
    update_timer: Timer,
    mic_area: Area,
}
```

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
pub struct App {
    ui: WidgetRef,
    ws_id: LiveId,                          // socket identity for web_socket_open
    asr_state: Option<Arc<DoubaoAsrState>>,
    app_id: String,
    access_token: String,
    audio_initialized: bool,
}
```

### Event handling

| Event | Action |
|-------|--------|
| `handle_startup` | Read env vars; init `DoubaoAsrState`; start audio |
| `handle_audio_devices` | `cx.use_audio_inputs()`; wire `cx.audio_input()` callback |
| `DoubaoAsrInputAction::RecordingStarted` | Build `HttpRequest` for `wss://...`; set auth headers; `cx.web_socket_open(ws_id, request)` |
| `DoubaoAsrInputAction::RecordingStopped` | Send last (empty) audio frame with `0x22` type |
| `Event::NetworkResponses::WsOpened` | Send config JSON binary frame |
| `Event::NetworkResponses::WsMessage` | Parse → update `confirmed_text` / `interim_text` |
| `Event::NetworkResponses::WsError` | Set `error` in state |
| `Event::NetworkResponses::WsClosed` | Set session state to Idle |
| Timer | Drain `pending_samples` → send audio frame |

### WebSocket authentication headers

```
X-Api-App-Key: {DOUBAO_APP_ID}
X-Api-Access-Key: {DOUBAO_ACCESS_TOKEN}
```

Set via `request.set_header()` before `cx.web_socket_open()`.

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

---

## 11. Error Handling

| Scenario | Behavior |
|---|---|
| Missing env vars at startup | Status: "Set DOUBAO_APP_ID and DOUBAO_ACCESS_TOKEN"; mic disabled |
| `WsError` from Makepad | State → Error; status label shows message; mic re-enabled after 1 timer tick |
| `code != 1000` in server JSON | Extract `message` field; show in status label; state → Idle |
| Recording < 0.1s audio | Don't open WebSocket; status: "Recording too short" |
| `WsClosed` during Streaming | Treat as error; re-enable mic |
| Parse failure on server frame | Log warning; ignore frame (don't crash) |

---

## 12. Testing Plan (Manual)

1. **Happy path:** Set env vars → run → mic click → speak Chinese → see interim gray text → click mic → final text in TextInput, interim clears, status "Ready"
2. **Missing credentials:** Run without env vars → mic button is dimmed; status shows credential instructions
3. **Network error:** Start recording → kill network → `WsError` appears in status, mic re-enables
4. **Very short recording:** Click mic, immediately click again → "Recording too short" message
5. **Multiple utterances:** Click mic → speak → click → speak → click → text accumulates in TextInput across sessions
