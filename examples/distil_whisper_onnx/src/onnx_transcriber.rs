// Sherpa-ONNX streaming Zipformer CTC transcription.
// Model: csukuangfj/sherpa-onnx-streaming-zipformer-ctc-zh-int8-2025-06-30
// Requires: model.int8.onnx + tokens.txt in model_dir.
// Download: https://huggingface.co/csukuangfj/sherpa-onnx-streaming-zipformer-ctc-zh-int8-2025-06-30

use ort::session::Session;
use ort::value::Tensor;

use crate::mel;
use crate::tokenizer::ZipformerTokenizer;

const DEFAULT_MODEL_DIR: &str = "/tmp/zipformer_ctc";
const DECODE_CHUNK_LEN: usize = 32;
const INPUT_T: usize = 45;   // total input window (32 new + 13 left context)
const VOCAB_SIZE: usize = 2000;
const EMBED_STATES_LEN: usize = 1 * 128 * 3 * 19; // 7296

pub fn find_model_dir() -> Option<String> {
    if let Ok(dir) = std::env::var("ZIPFORMER_MODEL_DIR") {
        if std::path::Path::new(&format!("{}/model.int8.onnx", dir)).exists() {
            return Some(dir);
        }
    }
    if std::path::Path::new(&format!("{}/model.int8.onnx", DEFAULT_MODEL_DIR)).exists() {
        return Some(DEFAULT_MODEL_DIR.to_string());
    }
    None
}

// Per-layer architecture parameters (N=1 substituted for batch dim).
struct LayerSpec {
    key_shape: [usize; 3],
    nonlin_attn_shape: [usize; 4],
    val_shape: [usize; 3],
    conv_shape: [usize; 3],
}

const LAYER_SPECS: &[LayerSpec] = &[
    // layers 0-1  (stack 0+1, dim=128)
    LayerSpec { key_shape: [128,1,128], nonlin_attn_shape: [1,1,128,192], val_shape: [128,1,48], conv_shape: [1,256,15] },
    LayerSpec { key_shape: [128,1,128], nonlin_attn_shape: [1,1,128,192], val_shape: [128,1,48], conv_shape: [1,256,15] },
    // layers 2-3  (stack 2+3, dim=64)
    LayerSpec { key_shape: [64,1,128],  nonlin_attn_shape: [1,1,64,288],  val_shape: [64,1,48],  conv_shape: [1,384,15] },
    LayerSpec { key_shape: [64,1,128],  nonlin_attn_shape: [1,1,64,288],  val_shape: [64,1,48],  conv_shape: [1,384,15] },
    // layers 4-7  (stack 4+5+6+7, dim=32)
    LayerSpec { key_shape: [32,1,128],  nonlin_attn_shape: [1,1,32,384],  val_shape: [32,1,48],  conv_shape: [1,512,7]  },
    LayerSpec { key_shape: [32,1,128],  nonlin_attn_shape: [1,1,32,384],  val_shape: [32,1,48],  conv_shape: [1,512,7]  },
    LayerSpec { key_shape: [32,1,128],  nonlin_attn_shape: [1,1,32,384],  val_shape: [32,1,48],  conv_shape: [1,512,7]  },
    LayerSpec { key_shape: [32,1,128],  nonlin_attn_shape: [1,1,32,384],  val_shape: [32,1,48],  conv_shape: [1,512,7]  },
    // layers 8-12 (stack 8..12, dim=16)
    LayerSpec { key_shape: [16,1,256],  nonlin_attn_shape: [1,1,16,576],  val_shape: [16,1,96],  conv_shape: [1,768,7]  },
    LayerSpec { key_shape: [16,1,256],  nonlin_attn_shape: [1,1,16,576],  val_shape: [16,1,96],  conv_shape: [1,768,7]  },
    LayerSpec { key_shape: [16,1,256],  nonlin_attn_shape: [1,1,16,576],  val_shape: [16,1,96],  conv_shape: [1,768,7]  },
    LayerSpec { key_shape: [16,1,256],  nonlin_attn_shape: [1,1,16,576],  val_shape: [16,1,96],  conv_shape: [1,768,7]  },
    LayerSpec { key_shape: [16,1,256],  nonlin_attn_shape: [1,1,16,576],  val_shape: [16,1,96],  conv_shape: [1,768,7]  },
    // layers 13-16 (stack 13..16, dim=32)
    LayerSpec { key_shape: [32,1,128],  nonlin_attn_shape: [1,1,32,384],  val_shape: [32,1,48],  conv_shape: [1,512,7]  },
    LayerSpec { key_shape: [32,1,128],  nonlin_attn_shape: [1,1,32,384],  val_shape: [32,1,48],  conv_shape: [1,512,7]  },
    LayerSpec { key_shape: [32,1,128],  nonlin_attn_shape: [1,1,32,384],  val_shape: [32,1,48],  conv_shape: [1,512,7]  },
    LayerSpec { key_shape: [32,1,128],  nonlin_attn_shape: [1,1,32,384],  val_shape: [32,1,48],  conv_shape: [1,512,7]  },
    // layers 17-18 (stack 17+18, dim=64, smaller attn dim)
    LayerSpec { key_shape: [64,1,128],  nonlin_attn_shape: [1,1,64,192],  val_shape: [64,1,48],  conv_shape: [1,256,15] },
    LayerSpec { key_shape: [64,1,128],  nonlin_attn_shape: [1,1,64,192],  val_shape: [64,1,48],  conv_shape: [1,256,15] },
];

fn product3(s: &[usize; 3]) -> usize { s[0] * s[1] * s[2] }
fn product4(s: &[usize; 4]) -> usize { s[0] * s[1] * s[2] * s[3] }

struct LayerState {
    key: Vec<f32>,
    nonlin_attn: Vec<f32>,
    val1: Vec<f32>,
    val2: Vec<f32>,
    conv1: Vec<f32>,
    conv2: Vec<f32>,
}

impl LayerState {
    fn new(spec: &LayerSpec) -> Self {
        Self {
            key: vec![0.0; product3(&spec.key_shape)],
            nonlin_attn: vec![0.0; product4(&spec.nonlin_attn_shape)],
            val1: vec![0.0; product3(&spec.val_shape)],
            val2: vec![0.0; product3(&spec.val_shape)],
            conv1: vec![0.0; product3(&spec.conv_shape)],
            conv2: vec![0.0; product3(&spec.conv_shape)],
        }
    }
}

struct ZipformerState {
    layers: Vec<LayerState>,
    embed_states: Vec<f32>,
}

impl ZipformerState {
    fn new() -> Self {
        Self {
            layers: LAYER_SPECS.iter().map(LayerState::new).collect(),
            embed_states: vec![0.0f32; EMBED_STATES_LEN],
        }
    }
}

fn shape3_i64(s: &[usize; 3]) -> Vec<i64> {
    s.iter().map(|&d| d as i64).collect()
}
fn shape4_i64(s: &[usize; 4]) -> Vec<i64> {
    s.iter().map(|&d| d as i64).collect()
}

fn t3(shape: &[usize; 3], data: Vec<f32>) -> Result<Tensor<f32>, String> {
    Tensor::<f32>::from_array((shape3_i64(shape), data)).map_err(|e| e.to_string())
}
fn t4(shape: &[usize; 4], data: Vec<f32>) -> Result<Tensor<f32>, String> {
    Tensor::<f32>::from_array((shape4_i64(shape), data)).map_err(|e| e.to_string())
}

// Extract mel window [t_len, 80] starting at frame `start`.
// mel is stored [80, N_FRAMES] row-major.
fn extract_mel_window(mel: &[f32], start: usize, t_len: usize) -> Vec<f32> {
    let n_frames = mel::N_FRAMES;
    let mut out = vec![0.0f32; t_len * mel::N_MELS];
    for t in 0..t_len {
        let src_t = (start + t).min(n_frames - 1);
        for m in 0..mel::N_MELS {
            out[t * mel::N_MELS + m] = mel[m * n_frames + src_t];
        }
    }
    out
}

fn run_chunk(
    session: &mut Session,
    x_data: Vec<f32>,
    state: &mut ZipformerState,
    processed_lens_val: i64,
) -> Result<Vec<Vec<f32>>, String> {
    let x_tensor = Tensor::<f32>::from_array((
        vec![1i64, INPUT_T as i64, mel::N_MELS as i64],
        x_data,
    )).map_err(|e| format!("x tensor: {}", e))?;

    let mut inputs = ort::inputs!["x" => x_tensor];

    for (i, ls) in LAYER_SPECS.iter().enumerate() {
        let s = &state.layers[i];
        inputs.push((format!("cached_key_{}", i).into(),          t3(&ls.key_shape, s.key.clone())?.into()));
        inputs.push((format!("cached_nonlin_attn_{}", i).into(),  t4(&ls.nonlin_attn_shape, s.nonlin_attn.clone())?.into()));
        inputs.push((format!("cached_val1_{}", i).into(),         t3(&ls.val_shape, s.val1.clone())?.into()));
        inputs.push((format!("cached_val2_{}", i).into(),         t3(&ls.val_shape, s.val2.clone())?.into()));
        inputs.push((format!("cached_conv1_{}", i).into(),        t3(&ls.conv_shape, s.conv1.clone())?.into()));
        inputs.push((format!("cached_conv2_{}", i).into(),        t3(&ls.conv_shape, s.conv2.clone())?.into()));
    }

    inputs.push(("embed_states".into(),
        Tensor::<f32>::from_array((vec![1i64, 128i64, 3i64, 19i64], state.embed_states.clone()))
            .map_err(|e| format!("embed_states tensor: {}", e))?.into()));

    inputs.push(("processed_lens".into(),
        Tensor::<i64>::from_array((vec![1i64], vec![processed_lens_val]))
            .map_err(|e| format!("processed_lens tensor: {}", e))?.into()));

    let outputs = session.run(inputs)
        .map_err(|e| format!("model run: {}", e))?;

    // Collect log_probs [1, T_out, 2000] → Vec<Vec<f32>>
    let log_probs_flat: Vec<f32> = {
        let (_, data) = outputs["log_probs"]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("extract log_probs: {}", e))?;
        data.to_vec()
    };
    let t_out = log_probs_flat.len() / VOCAB_SIZE;

    // Update per-layer states
    for i in 0..LAYER_SPECS.len() {
        let ls = &LAYER_SPECS[i];
        let s = &mut state.layers[i];

        let name = format!("new_cached_key_{}", i);
        s.key = {
            let (_, d) = outputs[name.as_str()].try_extract_tensor::<f32>()
                .map_err(|e| format!("extract {}: {}", name, e))?;
            if d.len() != product3(&ls.key_shape) {
                return Err(format!("{}: expected {} got {}", name, product3(&ls.key_shape), d.len()));
            }
            d.to_vec()
        };

        let name = format!("new_cached_nonlin_attn_{}", i);
        s.nonlin_attn = {
            let (_, d) = outputs[name.as_str()].try_extract_tensor::<f32>()
                .map_err(|e| format!("extract {}: {}", name, e))?;
            d.to_vec()
        };

        let name = format!("new_cached_val1_{}", i);
        s.val1 = {
            let (_, d) = outputs[name.as_str()].try_extract_tensor::<f32>()
                .map_err(|e| format!("extract {}: {}", name, e))?;
            d.to_vec()
        };

        let name = format!("new_cached_val2_{}", i);
        s.val2 = {
            let (_, d) = outputs[name.as_str()].try_extract_tensor::<f32>()
                .map_err(|e| format!("extract {}: {}", name, e))?;
            d.to_vec()
        };

        let name = format!("new_cached_conv1_{}", i);
        s.conv1 = {
            let (_, d) = outputs[name.as_str()].try_extract_tensor::<f32>()
                .map_err(|e| format!("extract {}: {}", name, e))?;
            d.to_vec()
        };

        let name = format!("new_cached_conv2_{}", i);
        s.conv2 = {
            let (_, d) = outputs[name.as_str()].try_extract_tensor::<f32>()
                .map_err(|e| format!("extract {}: {}", name, e))?;
            d.to_vec()
        };
    }

    state.embed_states = {
        let (_, d) = outputs["new_embed_states"]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("extract new_embed_states: {}", e))?;
        d.to_vec()
    };

    let mut result = Vec::with_capacity(t_out);
    for t in 0..t_out {
        result.push(log_probs_flat[t * VOCAB_SIZE..(t + 1) * VOCAB_SIZE].to_vec());
    }
    Ok(result)
}

/// Transcribe PCM audio (f32, 16 kHz mono) using the streaming Zipformer CTC ONNX model.
pub fn transcribe(samples: &[f32], model_dir: &str) -> Result<String, String> {
    let mut session = Session::builder()
        .map_err(|e| e.to_string())?
        .commit_from_file(format!("{}/model.int8.onnx", model_dir))
        .map_err(|e| format!("failed to load model: {}", e))?;

    let tokenizer = ZipformerTokenizer::load(&format!("{}/tokens.txt", model_dir))?;

    let mel = mel::log_mel_spectrogram(samples);
    let n_actual = ((samples.len() as f64 / mel::HOP_LENGTH as f64).ceil() as usize)
                   .min(mel::N_FRAMES);

    if n_actual == 0 {
        return Ok(String::new());
    }

    let mut state = ZipformerState::new();
    let mut all_time_steps: Vec<Vec<f32>> = Vec::new();

    let mut start = 0usize;
    while start < n_actual {
        let remaining = n_actual.saturating_sub(start);
        let chunk_len = remaining.min(DECODE_CHUNK_LEN) as i64;
        let x_data = extract_mel_window(&mel, start, INPUT_T);
        let time_steps = run_chunk(&mut session, x_data, &mut state, chunk_len)?;
        all_time_steps.extend(time_steps);
        start += DECODE_CHUNK_LEN;
    }

    // CTC argmax per time step
    let token_ids: Vec<i32> = all_time_steps.iter().map(|probs| {
        probs.iter().enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i as i32)
            .unwrap_or(0)
    }).collect();

    Ok(tokenizer.decode(&token_ids))
}
