// Mel spectrogram computation for Whisper (Slaney-style mel scale, matching openai/whisper)

pub const SAMPLE_RATE: usize = 16000;
pub const N_FFT: usize = 400;
pub const HOP_LENGTH: usize = 160;
pub const N_MELS: usize = 80;
pub const N_FRAMES: usize = 3000; // 30 seconds at 16kHz / 160 hop

struct MelCache {
    sin_vals: [f32; N_FFT],
    cos_vals: [f32; N_FFT],
    hann: [f32; N_FFT],
}

impl MelCache {
    fn new() -> Self {
        let mut sin_vals = [0.0f32; N_FFT];
        let mut cos_vals = [0.0f32; N_FFT];
        let mut hann = [0.0f32; N_FFT];

        for i in 0..N_FFT {
            let theta = 2.0 * std::f64::consts::PI * i as f64 / N_FFT as f64;
            sin_vals[i] = theta.sin() as f32;
            cos_vals[i] = theta.cos() as f32;
        }

        for i in 0..N_FFT {
            hann[i] = (0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / N_FFT as f64).cos())) as f32;
        }

        MelCache { sin_vals, cos_vals, hann }
    }
}

fn dft(input: &[f32], n: usize, out: &mut [f32], cache: &MelCache) {
    let step = N_FFT / n;
    for k in 0..n {
        let mut re = 0.0f32;
        let mut im = 0.0f32;
        for j in 0..n {
            let idx = (k * j * step) % N_FFT;
            re += input[j] * cache.cos_vals[idx];
            im -= input[j] * cache.sin_vals[idx];
        }
        out[k * 2] = re;
        out[k * 2 + 1] = im;
    }
}

fn fft(input: &mut [f32], n: usize, out: &mut [f32], cache: &MelCache) {
    if n == 1 {
        out[0] = input[0];
        out[1] = 0.0;
        return;
    }

    let half = n / 2;
    if n - half * 2 == 1 {
        dft(input, n, out, cache);
        return;
    }

    unsafe {
        let input_ptr = input.as_mut_ptr();
        let even_ptr = input_ptr.add(n);
        for i in 0..half {
            *even_ptr.add(i) = *input_ptr.add(2 * i);
        }
        let even_fft_ptr = out.as_mut_ptr().add(2 * n);
        let even_slice = std::slice::from_raw_parts_mut(even_ptr, half + 4 * half);
        let even_out = std::slice::from_raw_parts_mut(even_fft_ptr, 2 * half + 4 * half);
        fft(even_slice, half, even_out, cache);

        let odd_ptr = even_ptr;
        for i in 0..half {
            *odd_ptr.add(i) = *input_ptr.add(2 * i + 1);
        }
        let odd_fft_ptr = even_fft_ptr.add(n);
        let odd_slice = std::slice::from_raw_parts_mut(odd_ptr, half + 4 * half);
        let odd_out = std::slice::from_raw_parts_mut(odd_fft_ptr, 2 * half + 4 * half);
        fft(odd_slice, half, odd_out, cache);

        let step = N_FFT / n;
        for k in 0..half {
            let idx = k * step;
            let re = cache.cos_vals[idx];
            let im = -cache.sin_vals[idx];

            let re_odd = *odd_fft_ptr.add(2 * k);
            let im_odd = *odd_fft_ptr.add(2 * k + 1);

            let out_ptr = out.as_mut_ptr();
            *out_ptr.add(2 * k) = *even_fft_ptr.add(2 * k) + re * re_odd - im * im_odd;
            *out_ptr.add(2 * k + 1) = *even_fft_ptr.add(2 * k + 1) + re * im_odd + im * re_odd;
            *out_ptr.add(2 * (k + half)) = *even_fft_ptr.add(2 * k) - re * re_odd + im * im_odd;
            *out_ptr.add(2 * (k + half) + 1) = *even_fft_ptr.add(2 * k + 1) - re * im_odd - im * re_odd;
        }
    }
}

// Slaney-style mel scale (matches librosa default used by openai/whisper)
fn hz_to_mel_slaney(hz: f32) -> f32 {
    let f_sp = 200.0f32 / 3.0;
    let min_log_hz = 1000.0f32;
    let min_log_mel = min_log_hz / f_sp;
    let logstep = (6.4f32).ln() / 27.0;

    if hz < min_log_hz {
        hz / f_sp
    } else {
        min_log_mel + (hz / min_log_hz).ln() / logstep
    }
}

fn mel_to_hz_slaney(mel: f32) -> f32 {
    let f_sp = 200.0f32 / 3.0;
    let min_log_hz = 1000.0f32;
    let min_log_mel = min_log_hz / f_sp;
    let logstep = (6.4f32).ln() / 27.0;

    if mel < min_log_mel {
        mel * f_sp
    } else {
        min_log_hz * ((mel - min_log_mel) * logstep).exp()
    }
}

fn compute_mel_filters() -> Vec<f32> {
    let n_freqs = N_FFT / 2 + 1; // 201
    let fmax = SAMPLE_RATE as f32 / 2.0; // 8000 Hz

    let mel_min = hz_to_mel_slaney(0.0);
    let mel_max = hz_to_mel_slaney(fmax);

    let mel_points: Vec<f32> = (0..=N_MELS + 1)
        .map(|i| mel_min + (mel_max - mel_min) * i as f32 / (N_MELS + 1) as f32)
        .collect();

    let hz_points: Vec<f32> = mel_points.iter().map(|&m| mel_to_hz_slaney(m)).collect();

    // FFT bin for each mel point
    let bin_points: Vec<f32> = hz_points.iter()
        .map(|&hz| hz * N_FFT as f32 / SAMPLE_RATE as f32)
        .collect();

    let mut filters = vec![0.0f32; N_MELS * n_freqs];

    for m in 0..N_MELS {
        let f_lower = bin_points[m];
        let f_center = bin_points[m + 1];
        let f_upper = bin_points[m + 2];

        // Slaney area normalization
        let norm = if hz_points[m + 2] > hz_points[m] {
            2.0 / (hz_points[m + 2] - hz_points[m])
        } else {
            1.0
        };

        for k in 0..n_freqs {
            let f = k as f32;
            if f >= f_lower && f <= f_center && f_center > f_lower {
                filters[m * n_freqs + k] = norm * (f - f_lower) / (f_center - f_lower);
            } else if f > f_center && f <= f_upper && f_upper > f_center {
                filters[m * n_freqs + k] = norm * (f_upper - f) / (f_upper - f_center);
            }
        }
    }

    filters
}

/// Compute log-mel spectrogram from 16kHz mono PCM samples.
/// Returns flat Vec<f32> in [1, N_MELS, N_FRAMES] row-major order (C order).
/// If audio is shorter than 30s, the remaining frames are filled with log(1e-10).
pub fn log_mel_spectrogram(samples: &[f32]) -> Vec<f32> {
    let cache = MelCache::new();
    let filters = compute_mel_filters();
    let n_freqs = N_FFT / 2 + 1;

    let stage_1_pad = SAMPLE_RATE * 30; // 480000
    let stage_2_pad = N_FFT / 2; // 200

    let padded_len = samples.len() + stage_1_pad + stage_2_pad * 2;
    let mut padded = vec![0.0f32; padded_len];

    padded[stage_2_pad..stage_2_pad + samples.len()].copy_from_slice(samples);

    for i in 0..stage_2_pad.min(samples.len()) {
        padded[stage_2_pad - 1 - i] = samples[i + 1.min(samples.len() - 1)];
    }

    let n_len = (padded_len - N_FFT) / HOP_LENGTH;
    let n_mel_frames = n_len.min(N_FRAMES);

    let mut mel_data = vec![0.0f32; N_MELS * n_len];

    let n_samples_padded = samples.len() + stage_2_pad;
    let mut fft_in = vec![0.0f32; N_FFT * 4];
    let mut fft_out = vec![0.0f32; N_FFT * 8];

    for i in 0..n_len {
        let offset = i * HOP_LENGTH;

        for j in 0..N_FFT {
            if offset + j < n_samples_padded {
                fft_in[j] = cache.hann[j] * padded[offset + j];
            } else {
                fft_in[j] = 0.0;
            }
        }
        for j in N_FFT..fft_in.len() {
            fft_in[j] = 0.0;
        }

        fft(&mut fft_in, N_FFT, &mut fft_out, &cache);

        for j in 0..n_freqs {
            fft_out[j] = fft_out[2 * j] * fft_out[2 * j] + fft_out[2 * j + 1] * fft_out[2 * j + 1];
        }

        for j in 0..N_MELS {
            let mut sum = 0.0f64;
            let row = &filters[j * n_freqs..(j + 1) * n_freqs];
            let mut k = 0;
            while k + 3 < n_freqs {
                sum += fft_out[k] as f64 * row[k] as f64
                    + fft_out[k + 1] as f64 * row[k + 1] as f64
                    + fft_out[k + 2] as f64 * row[k + 2] as f64
                    + fft_out[k + 3] as f64 * row[k + 3] as f64;
                k += 4;
            }
            while k < n_freqs {
                sum += fft_out[k] as f64 * row[k] as f64;
                k += 1;
            }
            mel_data[j * n_len + i] = sum.max(1e-10).log10() as f32;
        }
    }

    let log_min = (1e-10f64).log10() as f32;
    for i in 0..n_len {
        if i * HOP_LENGTH >= n_samples_padded {
            for j in 0..N_MELS {
                mel_data[j * n_len + i] = log_min;
            }
        }
    }

    // Normalize: clamp, shift, scale — matches openai/whisper preprocessing
    let mut max_val = f32::NEG_INFINITY;
    for &v in mel_data.iter().take(N_MELS * n_mel_frames) {
        if v > max_val { max_val = v; }
    }
    max_val -= 8.0;

    for v in mel_data.iter_mut().take(N_MELS * n_mel_frames) {
        if *v < max_val { *v = max_val; }
        *v = (*v + 4.0) / 4.0;
    }

    // Build output [1, N_MELS, N_FRAMES] — reshape from [N_MELS, n_len] taking first N_FRAMES
    let mut out = vec![0.0f32; N_MELS * N_FRAMES];
    for m in 0..N_MELS {
        let src_start = m * n_len;
        let dst_start = m * N_FRAMES;
        let copy_len = n_mel_frames;
        out[dst_start..dst_start + copy_len].copy_from_slice(&mel_data[src_start..src_start + copy_len]);
        // remaining frames already zero (log(1e-10) would be negative; using 0 is fine for padding)
    }

    out
}
