use makepad_widgets::makepad_platform::audio::{AudioBuffer, AudioInfo};

use crate::decoder::DecodedPcm;

#[derive(Clone, Debug)]
pub struct PlayerState {
    pub cursor_frames: f64,
    pub playing: bool,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            cursor_frames: 0.0,
            playing: false,
        }
    }
}

pub fn fill_audio_output(
    state: &mut PlayerState,
    source: &DecodedPcm,
    info: AudioInfo,
    output: &mut AudioBuffer,
) {
    output.zero();

    if !state.playing || source.interleaved_samples.is_empty() {
        return;
    }

    let output_frames = output.frame_count();
    let output_channels = output.channel_count();
    let source_frames = source.frame_count();
    if source_frames == 0 || output_channels == 0 {
        state.playing = false;
        state.cursor_frames = 0.0;
        return;
    }

    let src_step = source.sample_rate as f64 / info.sample_rate;

    for frame in 0..output_frames {
        let src_pos = state.cursor_frames;
        if src_pos >= source_frames as f64 {
            state.playing = false;
            state.cursor_frames = 0.0;
            break;
        }

        let src_idx = src_pos.floor() as usize;
        let frac = (src_pos - src_idx as f64) as f32;
        let next_idx = (src_idx + 1).min(source_frames - 1);

        let left = interpolate_sample(source, src_idx, next_idx, frac, 0);
        let right = interpolate_sample(source, src_idx, next_idx, frac, 1);

        output.data[frame] = left;
        if output_channels > 1 {
            output.data[frame + output_frames] = right;
        }
        for channel in 2..output_channels {
            output.data[channel * output_frames + frame] = 0.5 * (left + right);
        }

        state.cursor_frames += src_step;
    }

    if state.cursor_frames >= source_frames as f64 {
        state.playing = false;
        state.cursor_frames = 0.0;
    }
}

fn interpolate_sample(
    source: &DecodedPcm,
    src_idx: usize,
    next_idx: usize,
    frac: f32,
    channel: usize,
) -> f32 {
    let current = source.interleaved_samples[src_idx * 2 + channel];
    let next = source.interleaved_samples[next_idx * 2 + channel];
    current + (next - current) * frac
}

#[cfg(test)]
mod tests {
    use makepad_widgets::makepad_platform::audio::{AudioDeviceId, AudioInfo};

    use super::*;

    fn source(sample_rate: u32, frame_count: usize) -> DecodedPcm {
        let mut interleaved_samples = Vec::with_capacity(frame_count * 2);
        for frame in 0..frame_count {
            interleaved_samples.push(frame as f32);
            interleaved_samples.push(-(frame as f32));
        }
        DecodedPcm {
            sample_rate,
            channels: 2,
            interleaved_samples,
        }
    }

    fn info(sample_rate: f64) -> AudioInfo {
        AudioInfo {
            device_id: AudioDeviceId::default(),
            time: None,
            sample_rate,
        }
    }

    #[test]
    fn test_paused_player_does_not_advance_cursor() {
        let mut state = PlayerState {
            playing: false,
            cursor_frames: 100.0,
        };
        let source = source(44_100, 5_000);
        let mut output = AudioBuffer::new_with_size(512, 2);

        fill_audio_output(&mut state, &source, info(48_000.0), &mut output);

        assert_eq!(state.cursor_frames, 100.0);
        assert!(output.data.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn test_player_stops_and_resets_at_end_of_source() {
        let mut state = PlayerState {
            playing: true,
            cursor_frames: 95.0,
        };
        let source = source(44_100, 100);
        let mut output = AudioBuffer::new_with_size(32, 2);

        fill_audio_output(&mut state, &source, info(44_100.0), &mut output);

        assert!(!state.playing);
        assert_eq!(state.cursor_frames, 0.0);
        assert!(output.channel(0)[..5].iter().any(|sample| *sample != 0.0));
        assert!(output.channel(1)[..5].iter().any(|sample| *sample != 0.0));
        assert!(output.channel(0)[5..].iter().all(|sample| *sample == 0.0));
        assert!(output.channel(1)[5..].iter().all(|sample| *sample == 0.0));
    }
}
