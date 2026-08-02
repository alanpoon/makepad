//! Writes the four head planes to /tmp so they can be diffed against the
//! TFLite reference. Skips unless the converted model is present.
use makepad_example_pose_movenet::{image_io, movenet::Estimator};

#[test]
fn dump_heads_for_reference_comparison() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if !dir.join("model/movenet.json").exists() {
        return;
    }
    let Ok(est) = Estimator::load(dir.join("model")) else { return };
    let bytes = std::fs::read(dir.join("assets/pose.jpg")).unwrap();
    let img = image_io::decode(&bytes).unwrap();
    let heads = est.debug_heads(&img.rgb, img.width, img.height).unwrap();
    for (name, plane) in [
        ("heatmap", &heads.0),
        ("center", &heads.1),
        ("regress", &heads.2),
        ("offset", &heads.3),
    ] {
        let raw: Vec<u8> = plane.iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(format!("/tmp/rust_{name}.f32"), raw).unwrap();
    }
    eprintln!("wrote /tmp/rust_*.f32, grid {:?}", est.grid());
}
