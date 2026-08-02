//! The two-stage MediaPipe pipeline: palm detection, then one landmark pass
//! per detected hand.

use crate::hand::{Hand, HandLandmarker};
use crate::palm::{DetectParams, Palm, PalmDetector};
use crate::roi::{crop_rgb8, crop_to_source, roi_for, RotatedRect};
use makepad_nn_graph::NnError;
use std::path::Path;

/// One hand: where the detector found it, the crop that was fed to the
/// landmark model, and the landmarks mapped back onto the source image.
#[derive(Clone, Debug)]
pub struct TrackedHand {
    pub palm: Palm,
    pub roi: RotatedRect,
    pub hand: Hand,
}

pub struct HandPipeline {
    detector: PalmDetector,
    landmarker: HandLandmarker,
}

impl HandPipeline {
    pub fn load(dir: impl AsRef<Path>) -> Result<Self, NnError> {
        let dir = dir.as_ref();
        Ok(Self {
            detector: PalmDetector::load(dir)?,
            landmarker: HandLandmarker::load(dir)?,
        })
    }

    pub fn params(&self) -> DetectParams {
        self.detector.params
    }

    pub fn set_params(&mut self, params: DetectParams) {
        self.detector.params = params;
    }

    /// Detect every hand in the frame and land 21 landmarks on each.
    pub fn run(
        &self,
        rgb: &[u8],
        width: usize,
        height: usize,
    ) -> Result<Vec<TrackedHand>, NnError> {
        let palms = self.detector.detect(rgb, width, height)?;

        let mut out = Vec::with_capacity(palms.len());
        for palm in palms {
            let roi = roi_for(&palm, width, height);
            let size = self.landmarker.input_size();
            let planes = crop_rgb8(
                rgb,
                width,
                height,
                &roi,
                size,
                self.landmarker.input_scale(),
                self.landmarker.input_bias(),
            )
            .map_err(NnError::Input)?;

            let mut hand = self.landmarker.detect_planar(&planes)?;
            // crop space -> source image
            for lm in hand.landmarks.iter_mut() {
                let p = crop_to_source(&roi, lm.x, lm.y, width, height);
                lm.x = p.x;
                lm.y = p.y;
            }
            out.push(TrackedHand { palm, roi, hand });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palm::{Point, NUM_PALM_KEYPOINTS};
    use crate::roi::roi_for;

    /// The ROI must cover the palm box it came from, or the landmark model
    /// sees a hand cut in half.
    #[test]
    fn roi_contains_the_detection() {
        let mut keypoints = [Point::default(); NUM_PALM_KEYPOINTS];
        keypoints[0] = Point { x: 0.5, y: 0.62 };
        keypoints[2] = Point { x: 0.5, y: 0.42 };
        let palm = Palm {
            score: 0.9,
            center: Point { x: 0.5, y: 0.5 },
            width: 0.15,
            height: 0.15,
            keypoints,
        };
        let (w, h) = (400usize, 400usize);
        let roi = roi_for(&palm, w, h);

        // corners of the palm box, in crop coordinates
        for (px, py) in [
            (palm.left(), palm.top()),
            (palm.right(), palm.top()),
            (palm.left(), palm.bottom()),
            (palm.right(), palm.bottom()),
        ] {
            // invert crop_to_source for an unrotated ROI
            let lx = (px - roi.center.x) * w as f32 / roi.size_px + 0.5;
            let ly = (py - roi.center.y) * h as f32 / roi.size_px + 0.5;
            assert!(
                (0.0..=1.0).contains(&lx) && (0.0..=1.0).contains(&ly),
                "palm corner ({px}, {py}) fell outside the crop at ({lx}, {ly})"
            );
        }
    }
}
