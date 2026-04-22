//! SpeechInput - A standalone speech-to-text widget for Makepad
//!
//! # Example Usage
//! ```
//! speech_input := SpeechInput {
//!     placeholder: "Speak or type..."
//!     accent_color: #FF6600
//! }
//! ```

use makepad_widgets::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const WHISPER_SAMPLE_RATE: f64 = 16000.0;
const MAX_RECENT_SAMPLES: usize = 1600; // 100ms at 16kHz

// ============================================================================
// Recording State (shared between audio thread and UI)
// ============================================================================

pub struct SpeechRecordingState {
    pub is_recording: AtomicBool,
    pub accumulated_samples: Mutex<Vec<f32>>,
    pub sample_rate: Mutex<f64>,
    pub transcription_result: Mutex<Option<String>>,
    pub recent_samples: Mutex<Vec<f32>>,
    pub error: Mutex<Option<String>>,
}

impl SpeechRecordingState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            is_recording: AtomicBool::new(false),
            accumulated_samples: Mutex::new(Vec::new()),
            sample_rate: Mutex::new(44100.0),
            transcription_result: Mutex::new(None),
            recent_samples: Mutex::new(Vec::new()),
            error: Mutex::new(None),
        })
    }

    pub fn start_recording(&self) {
        self.accumulated_samples.lock().unwrap().clear();
        self.recent_samples.lock().unwrap().clear();
        self.is_recording.store(true, Ordering::SeqCst);
    }

    pub fn stop_recording(&self) {
        self.is_recording.store(false, Ordering::SeqCst);
    }

    pub fn clear(&self) {
        self.accumulated_samples.lock().unwrap().clear();
        self.recent_samples.lock().unwrap().clear();
        self.is_recording.store(false, Ordering::SeqCst);
        *self.transcription_result.lock().unwrap() = None;
        *self.error.lock().unwrap() = None;
    }

    pub fn calculate_amplitude(&self) -> f32 {
        let recent = self.recent_samples.lock().unwrap();
        if recent.is_empty() {
            0.0
        } else {
            let rms: f32 = (recent.iter().map(|s| s * s).sum::<f32>() / recent.len() as f32).sqrt();
            (rms * 30.0).min(1.0)
        }
    }

    pub fn get_samples(&self) -> Vec<f32> {
        self.accumulated_samples.lock().unwrap().clone()
    }

    pub fn take_result(&self) -> Option<String> {
        self.transcription_result.lock().unwrap().take()
    }

    pub fn take_error(&self) -> Option<String> {
        self.error.lock().unwrap().take()
    }
}

// ============================================================================
// Audio Processing
// ============================================================================

pub fn resample_to_16k_mono(input: &makepad_widgets::makepad_platform::audio::AudioBuffer, from_rate: f64) -> Vec<f32> {
    if input.frame_count() == 0 {
        return Vec::new();
    }

    let ratio = WHISPER_SAMPLE_RATE / from_rate;
    let new_len = ((input.frame_count() as f64 * ratio).round() as usize).max(1);
    let mut output = vec![0.0f32; new_len];
    let channel_count = input.channel_count().max(1) as f32;

    for i in 0..new_len {
        let src_pos = i as f64 / ratio;
        let src_idx = src_pos as usize;
        let frac = (src_pos - src_idx as f64) as f32;

        let mut sample0 = 0.0f32;
        let mut sample1 = 0.0f32;
        for ch in 0..input.channel_count() {
            sample0 += input.channel(ch).get(src_idx).copied().unwrap_or(0.0);
            sample1 += input.channel(ch).get(src_idx + 1).copied().unwrap_or(0.0);
        }
        sample0 /= channel_count;
        sample1 /= channel_count;
        if sample1 == 0.0 { sample1 = sample0; }

        output[i] = sample0 + (sample1 - sample0) * frac;
    }
    output
}

pub fn process_audio_input(state: &Arc<SpeechRecordingState>, info: makepad_widgets::makepad_platform::audio::AudioInfo, input_buffer: &makepad_widgets::makepad_platform::audio::AudioBuffer) {
    *state.sample_rate.lock().unwrap() = info.sample_rate;
    let resampled = resample_to_16k_mono(input_buffer, info.sample_rate);

    // Update recent samples for waveform visualization
    {
        let mut recent = state.recent_samples.lock().unwrap();
        recent.extend(resampled.iter());
        let len = recent.len();
        if len > MAX_RECENT_SAMPLES {
            recent.drain(0..len - MAX_RECENT_SAMPLES);
        }
    }

    // Only accumulate samples when recording
    if state.is_recording.load(Ordering::SeqCst) {
        state.accumulated_samples.lock().unwrap().extend(resampled.iter());
    }
}

// ============================================================================
// Transcription
// ============================================================================

fn find_model_path() -> Option<String> {
    if let Ok(path) = std::env::var("MAKEPAD_VOICE_MODEL") {
        if std::path::Path::new(&path).exists() {
            return Some(path);
        }
    }

    let candidates = [
        "ggml-large-v3-turbo.bin",
        "../../ggml-large-v3-turbo.bin",
        "../../../ggml-large-v3-turbo.bin",
    ];

    for candidate in candidates {
        if std::path::Path::new(candidate).exists() {
            return Some(candidate.to_string());
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            for ancestor in exe_dir.ancestors().take(5) {
                let model_path = ancestor.join("ggml-large-v3-turbo.bin");
                if model_path.exists() {
                    return model_path.to_str().map(|s| s.to_string());
                }
            }
        }
    }

    None
}

pub fn transcribe_async(state: Arc<SpeechRecordingState>) {
    let samples = state.get_samples();
    if samples.is_empty() {
        return;
    }

    std::thread::spawn(move || {
        let model_path = match find_model_path() {
            Some(path) => path,
            None => {
                *state.error.lock().unwrap() = Some("Model not found. Set MAKEPAD_VOICE_MODEL".to_string());
                return;
            }
        };

        match makepad_voice::WhisperModel::load_file(&model_path) {
            Ok(model) => {
                let mut whisper_state = makepad_voice::WhisperState::new(&model);
                let params = makepad_voice::WhisperParams::default();
                let segments = whisper_state.transcribe(&model, &samples, &params);
                let text: String = segments.iter()
                    .map(|s| s.text.trim())
                    .collect::<Vec<_>>()
                    .join(" ");
                *state.transcription_result.lock().unwrap() = Some(text);
            }
            Err(e) => {
                *state.error.lock().unwrap() = Some(format!("Failed to load model: {:?}", e));
            }
        }
    });
}

// ============================================================================
// Draw Structs
// ============================================================================

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

// ============================================================================
// SpeechInput Widget
// ============================================================================

#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub enum SpeechInputAction {
    #[default]
    None,
    RecordingStarted,
    RecordingStopped,
    TranscriptionStarted,
    TranscriptionComplete(String),
    TranscriptionError(String),
    TextChanged(String),
}

#[derive(Script, ScriptHook, Widget)]
pub struct SpeechInput {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    // Inner widgets - found by id
    #[find]
    #[redraw]
    #[live]
    text_input: WidgetRef,

    // Mic button drawing
    #[redraw]
    #[live]
    draw_mic: DrawMicButton,

    // Spinner drawing
    #[redraw]
    #[live]
    draw_spinner: DrawSpinner,

    // Background
    #[redraw]
    #[live]
    draw_bg: DrawQuad,

    // Visibility
    #[live(true)]
    #[visible]
    visible: bool,

    // Customization - Colors
    #[live]
    pub accent_color: Vec4,

    // Customization - Sizes
    #[live(40.0)]
    pub mic_button_size: f64,

    // Runtime state
    #[rust]
    state: Option<Arc<SpeechRecordingState>>,
    #[rust]
    current_amplitude: f32,
    #[rust]
    recording_start_time: Option<std::time::Instant>,
    #[rust]
    is_transcribing: bool,
    #[rust]
    update_timer: Timer,
    #[rust]
    mic_area: Area,
}

impl Widget for SpeechInput {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.visible {
            return DrawStep::done();
        }

        // Set shader properties
        self.draw_mic.accent_color = self.accent_color;

        let button_walk = Walk::fixed(self.mic_button_size, self.mic_button_size);

        // Draw either mic button or loading spinner
        if self.is_transcribing {
            self.draw_spinner.time = cx.time() as f32;
            self.draw_spinner.draw_walk(cx, button_walk);
        } else {
            self.draw_mic.draw_walk(cx, button_walk);
            self.mic_area = self.draw_mic.area();
        }

        // Draw the text input
        self.text_input.draw_walk(cx, scope, walk);

        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if !self.visible {
            return;
        }

        // Handle timer for amplitude updates
        if let Event::Timer(te) = event {
            if self.update_timer.is_timer(te).is_some() {
                self.update_state(cx);
            }
        }

        // Handle mic button clicks
        if let Hit::FingerDown(_) = event.hits(cx, self.mic_area) {
            self.toggle_recording(cx);
        }
    }
}

#[allow(dead_code)]
impl SpeechInput {
    /// Initialize the widget (call from app startup)
    pub fn init(&mut self, cx: &mut Cx) {
        self.state = Some(SpeechRecordingState::new());
        self.update_timer = cx.start_interval(0.033); // ~30fps
    }

    /// Get the recording state for audio processing
    pub fn recording_state(&self) -> Option<Arc<SpeechRecordingState>> {
        self.state.clone()
    }

    /// Toggle recording on/off
    pub fn toggle_recording(&mut self, cx: &mut Cx) {
        let state = match &self.state {
            Some(s) => s.clone(),
            None => return,
        };

        let was_recording = state.is_recording.load(Ordering::SeqCst);

        if was_recording {
            // Stop recording and start transcription
            state.stop_recording();
            self.recording_start_time = None;
            self.draw_mic.is_recording = 0.0;

            if !state.get_samples().is_empty() {
                self.is_transcribing = true;
                transcribe_async(state);
                cx.widget_action(self.widget_uid(), SpeechInputAction::TranscriptionStarted);
            }

            cx.widget_action(self.widget_uid(), SpeechInputAction::RecordingStopped);
        } else {
            // Start recording
            state.start_recording();
            self.recording_start_time = Some(std::time::Instant::now());
            self.draw_mic.is_recording = 1.0;

            cx.widget_action(self.widget_uid(), SpeechInputAction::RecordingStarted);
        }

        self.redraw(cx);
    }

    /// Update internal state (called on timer)
    fn update_state(&mut self, cx: &mut Cx) {
        let state = match &self.state {
            Some(s) => s.clone(),
            None => return,
        };

        // Calculate smoothed amplitude
        let amplitude = state.calculate_amplitude();
        self.current_amplitude = self.current_amplitude * 0.7 + amplitude * 0.3;
        self.draw_mic.amplitude = self.current_amplitude;

        // Check for transcription results
        if let Some(result) = state.take_result() {
            self.is_transcribing = false;
            if !result.is_empty() {
                // Append to existing text in the inner TextInput
                let current = self.text();
                let new_text = if current.is_empty() {
                    result.clone()
                } else {
                    format!("{} {}", current, result)
                };
                self.set_text(cx, &new_text);

                cx.widget_action(self.widget_uid(), SpeechInputAction::TranscriptionComplete(result));
                cx.widget_action(self.widget_uid(), SpeechInputAction::TextChanged(new_text));
            }
        }

        // Check for errors
        if let Some(error) = state.take_error() {
            self.is_transcribing = false;
            cx.widget_action(self.widget_uid(), SpeechInputAction::TranscriptionError(error));
        }

        self.redraw(cx);
    }

    /// Get current transcribed text
    pub fn text(&self) -> String {
        self.text_input.text()
    }

    /// Set transcribed text
    pub fn set_text(&mut self, cx: &mut Cx, text: &str) {
        self.text_input.set_text(cx, text);
    }

    /// Clear all text and state
    pub fn clear(&mut self, cx: &mut Cx) {
        if let Some(state) = &self.state {
            state.clear();
        }
        self.set_text(cx, "");
        self.current_amplitude = 0.0;
        self.is_transcribing = false;
        self.recording_start_time = None;
        self.draw_mic.is_recording = 0.0;
        self.draw_mic.amplitude = 0.0;
        self.redraw(cx);
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.state
            .as_ref()
            .map(|s| s.is_recording.load(Ordering::SeqCst))
            .unwrap_or(false)
    }

    /// Check if currently transcribing
    pub fn is_transcribing(&self) -> bool {
        self.is_transcribing
    }

    /// Get recording duration if recording
    pub fn recording_duration(&self) -> Option<std::time::Duration> {
        self.recording_start_time.map(|t| t.elapsed())
    }

    /// Handle actions from this widget
    pub fn handle_action(&self, actions: &Actions) -> Option<SpeechInputAction> {
        actions.find_widget_action(self.widget_uid())
            .map(|a| a.cast::<SpeechInputAction>())
    }
}

impl WidgetMatchEvent for SpeechInput {
    fn handle_actions(&mut self, _cx: &mut Cx, _actions: &Actions, _scope: &mut Scope) {}
}
