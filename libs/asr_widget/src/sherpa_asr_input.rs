use makepad_widgets::*;
use makepad_widgets::makepad_platform::audio::{AudioInfo, AudioBuffer};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use sherpa_onnx::{OnlineRecognizer, OnlineStream};

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
