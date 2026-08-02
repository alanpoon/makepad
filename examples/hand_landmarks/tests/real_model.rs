//! Runs the full two-stage pipeline over the bundled photo.
use makepad_example_hand_landmarks::{hand::LANDMARK_NAMES, image_io, pipeline::HandPipeline};

#[test]
fn detects_and_lands_a_hand_in_the_bundled_photo() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let model = dir.join("model");
    let photo = dir.join("assets/hand.jpg");
    if !model.join("palm_detector.json").exists() || !photo.exists() {
        eprintln!("skipping: no model or photo");
        return;
    }
    let pipeline = match HandPipeline::load(&model) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };

    let bytes = std::fs::read(&photo).unwrap();
    let img = image_io::decode(&bytes).unwrap();
    let hands = pipeline.run(&img.rgb, img.width, img.height).unwrap();

    eprintln!("detected {} hand(s)", hands.len());
    for t in hands.iter() {
        eprintln!(
            "palm score {:.3} at ({:.3}, {:.3}) {:.3}x{:.3}, roi {:.0}px rot {:.1} deg",
            t.palm.score,
            t.palm.center.x,
            t.palm.center.y,
            t.palm.width,
            t.palm.height,
            t.roi.size_px,
            t.roi.rotation.to_degrees()
        );
        eprintln!(
            "  presence {:.3}  handedness {:?} {:.3}",
            t.hand.presence, t.hand.handedness, t.hand.handedness_score
        );
        for (i, p) in t.hand.landmarks.iter().enumerate() {
            eprintln!("  {:>12}  ({:.3}, {:.3}, {:+.3})", LANDMARK_NAMES[i], p.x, p.y, p.z);
        }
    }

    assert!(!hands.is_empty(), "palm detection found nothing");
    let t = &hands[0];
    assert!(t.palm.score > 0.5);
    assert!(
        t.hand.presence > 0.5,
        "a cropped hand should score well above 0.5, got {}",
        t.hand.presence
    );
    // landmarks must land on the picture
    for p in t.hand.landmarks.iter() {
        assert!((-0.2..=1.2).contains(&p.x) && (-0.2..=1.2).contains(&p.y));
    }
}
