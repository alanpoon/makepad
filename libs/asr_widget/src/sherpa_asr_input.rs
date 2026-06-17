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
