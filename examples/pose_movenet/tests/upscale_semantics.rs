//! Pins ggml's bilinear upscale against TensorFlow's half_pixel_centers rule,
//! which is what the MoveNet FPN uses.
use makepad_ggml::backend::metal::{
    prepare_graph, BufferStorageMode, MetalGraphSession, MetalRuntime,
};
use makepad_ggml::{BufferUsage, Context, Graph, InitParams, ScaleMode, TensorType};

#[test]
fn bilinear_upscale_matches_tensorflow_half_pixel() {
    let Ok(runtime) = MetalRuntime::new() else {
        return;
    };
    let mut ctx = Context::new(InitParams {
        mem_size: 1 << 20,
        mem_buffer: None,
        no_alloc: false,
    });

    let src = ctx
        .new_tensor_4d(TensorType::F32, 3, 3, 1, 1, BufferUsage::Activations)
        .unwrap();
    let values: Vec<f32> = (0..9).map(|v| v as f32).collect();
    let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
    ctx.write_tensor_data(src, &bytes).unwrap();

    let out = ctx
        .upscale(src, 2, ScaleMode::Bilinear, false, false, BufferUsage::Activations)
        .unwrap();

    let mut g = Graph::new();
    g.build_forward_expand(&ctx, out).unwrap();
    let prepared = prepare_graph(&ctx, &g, runtime.features()).unwrap();
    let session = MetalGraphSession::from_runtime(
        runtime,
        &ctx,
        &prepared,
        BufferStorageMode::Shared,
        BufferStorageMode::Shared,
    )
    .unwrap();
    let exec = session.execute(&ctx, &[], &[out]).unwrap();
    let got: Vec<f32> = exec.outputs[&out]
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    // TF: src = clamp((dst + 0.5)/2 - 0.5, 0, 2) -> 0, 0.25, 0.75, 1.25, 1.75, 2
    let coords = [0.0f32, 0.25, 0.75, 1.25, 1.75, 2.0];
    let mut want = Vec::new();
    for y in coords {
        for x in coords {
            let (y0, x0) = (y.floor(), x.floor());
            let (y1, x1) = ((y0 + 1.0).min(2.0), (x0 + 1.0).min(2.0));
            let (fy, fx) = (y - y0, x - x0);
            let at = |r: f32, c: f32| values[(r as usize) * 3 + c as usize];
            let top = at(y0, x0) * (1.0 - fx) + at(y0, x1) * fx;
            let bot = at(y1, x0) * (1.0 - fx) + at(y1, x1) * fx;
            want.push(top * (1.0 - fy) + bot * fy);
        }
    }

    for row in 0..6 {
        let g: Vec<String> = (0..6).map(|c| format!("{:5.2}", got[row * 6 + c])).collect();
        let w: Vec<String> = (0..6).map(|c| format!("{:5.2}", want[row * 6 + c])).collect();
        eprintln!("ggml [{}]   tf [{}]", g.join(" "), w.join(" "));
    }
    for (i, (g, w)) in got.iter().zip(want.iter()).enumerate() {
        assert!(
            (g - w).abs() < 1e-4,
            "cell {i}: ggml {g} != tensorflow {w}"
        );
    }
}
