use makepad_widgets::*;
use makepad_widgets::makepad_platform::makepad_network::{NetworkRuntime, WsSend};
use makepad_micro_serde::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

// ── Protocol constants ────────────────────────────────────────────────────────

const PROTO_HEADER_BYTE: u8 = 0x11;
const MSG_FULL_CLIENT_REQUEST: u8 = 0x10;
const MSG_AUDIO_ONLY: u8 = 0x20;
const MSG_AUDIO_LAST: u8 = 0x22;
const SERIALIZATION_JSON: u8 = 0x10;
const SERIALIZATION_RAW: u8 = 0x00;

// ── State ────────────────────────────────────────────────────────────────────

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
        let mut guard = self.session.lock().unwrap();
        if let SessionState::Error(msg) = &*guard {
            let msg = msg.clone();
            *guard = SessionState::Idle;
            Some(msg)
        } else {
            None
        }
    }
}

// ── Audio helpers ─────────────────────────────────────────────────────────────

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

// ── Protocol frame builders ───────────────────────────────────────────────────

fn build_frame(msg_type: u8, serialization: u8, payload: &[u8]) -> Vec<u8> {
    let size = payload.len() as u32;
    let mut frame = Vec::with_capacity(8 + payload.len());
    frame.push(PROTO_HEADER_BYTE);
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
    build_frame(MSG_FULL_CLIENT_REQUEST, SERIALIZATION_JSON, json.as_bytes())
}

pub fn build_audio_frame(pcm_bytes: &[u8], is_last: bool) -> Vec<u8> {
    let msg_type = if is_last { MSG_AUDIO_LAST } else { MSG_AUDIO_ONLY };
    build_frame(msg_type, SERIALIZATION_RAW, pcm_bytes)
}

// ── Response parser ───────────────────────────────────────────────────────────

#[derive(DeJson, Default)]
pub struct DoubaoResult {
    pub text: Option<String>,
    pub is_final: Option<bool>,
}

#[derive(DeJson, Default)]
pub struct DoubaoResponse {
    pub code: Option<i64>,
    pub message: Option<String>,
    pub result: Option<DoubaoResult>,
}

pub fn parse_response_frame(data: &[u8]) -> Option<DoubaoResponse> {
    if data.len() < 8 { return None; }
    let payload = &data[8..];
    let json_str = std::str::from_utf8(payload).ok()?;
    DoubaoResponse::deserialize_json(json_str).ok()
}

// ── Draw structs ──────────────────────────────────────────────────────────────

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

// ── Widget ────────────────────────────────────────────────────────────────────

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
        self.update_timer = cx.start_interval(0.033);
    }

    pub fn handle_action(&self, actions: &Actions) -> Option<DoubaoAsrInputAction> {
        actions
            .find_widget_action(self.widget_uid())
            .map(|a| a.cast::<DoubaoAsrInputAction>())
    }

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

    fn update_ui(&mut self, cx: &mut Cx) {
        let state = match &self.state { Some(s) => s.clone(), None => return };

        // Auto-transition: Error → Idle on next timer tick (status label already set by App)
        let _ = state.take_error_msg();

        let amplitude = state.calculate_amplitude();
        self.current_amplitude = self.current_amplitude * 0.7 + amplitude * 0.3;
        self.draw_mic.amplitude = self.current_amplitude;
        self.draw_mic.is_recording = if state.is_recording.load(Ordering::SeqCst) { 1.0 } else { 0.0 };

        let interim = state.interim_text.lock().unwrap().clone();
        self.interim_label.set_text(cx, &interim);

        self.redraw(cx);
    }
}

impl WidgetMatchEvent for DoubaoAsrInput {
    fn handle_actions(&mut self, _cx: &mut Cx, _actions: &Actions, _scope: &mut Scope) {}
}

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

        let _ = self.text_input.draw_walk(cx, scope, walk);
        let _ = self.interim_label.draw_walk(cx, scope, Walk::default());

        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if !self.visible { return; }

        if let Event::Timer(te) = event {
            if self.update_timer.is_timer(te).is_some() {
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
}
