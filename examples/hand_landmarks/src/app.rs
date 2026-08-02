//! Loads an image, runs the MediaPipe hand landmark model, and draws the
//! 21 landmarks with one color per finger.
//!
//!   cargo run -p makepad-example-hand-landmarks -- <image> [model-dir]
//!
//! Defaults to `./hand.jpg` and `examples/hand_landmarks/model`.

use makepad_widgets::*;

use crate::hand::{Handedness, FINGERTIPS, LANDMARK_NAMES};
use crate::pipeline::{HandPipeline, TrackedHand};
use crate::image_io;
use crate::overlay::{draw_hand, draw_handedness, rgba8_to_bgra_u32, Canvas};

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                body +: {
                    main_view := View{
                        width: Fill
                        height: Fill
                        flow: Down
                        spacing: 10
                        padding: 14

                        Label{
                            text: "Hand landmarks"
                            draw_text.text_style.font_size: 18
                        }

                        status_label := Label{
                            text: "Loading..."
                            draw_text.text_style.font_size: 10
                            draw_text.color: #999
                        }

                        controls := View{
                            width: Fill
                            height: Fit
                            flow: Right
                            spacing: 8

                            reload_btn := Button{ text: "Reload" }
                            toggle_btn := Button{ text: "Hide landmarks" }
                        }

                        image_host := View{
                            width: Fill
                            height: Fill
                            align: Center
                            hand_image := Image{
                                width: Fill
                                height: Fill
                                fit: ImageFit.Smallest
                            }
                        }

                        tips_label := Label{
                            text: ""
                            draw_text.text_style.font_size: 9
                            draw_text.color: #888
                        }
                    }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
    #[rust]
    state: AppState,
}

#[derive(Default)]
struct AppState {
    image_path: String,
    model_dir: String,
    model: Option<HandPipeline>,
    source: Option<image_io::DecodedImage>,
    hands: Vec<TrackedHand>,
    show_landmarks: bool,
}

/// Below this the landmark model is telling us its ROI does not hold a hand.
/// With palm detection feeding it a tight crop, real hands score well above
/// this.
const MIN_PRESENCE: f32 = 0.5;

impl App {
    fn args() -> (String, String) {
        let mut args = std::env::args().skip(1).filter(|a| !a.starts_with('-'));
        let image = args.next().unwrap_or_else(|| "hand.jpg".to_string());
        let model = args
            .next()
            .unwrap_or_else(|| "examples/hand_landmarks/model".to_string());
        (image, model)
    }

    fn reload(&mut self, cx: &mut Cx) {
        let (image_path, model_dir) = Self::args();
        self.state.image_path = image_path;
        self.state.model_dir = model_dir;
        self.state.hands.clear();

        let bytes = match std::fs::read(&self.state.image_path) {
            Ok(b) => b,
            Err(e) => {
                self.state.source = None;
                self.set_status(
                    cx,
                    &format!(
                        "cannot read image {}: {e}\nusage: <image.jpg> [model-dir]",
                        self.state.image_path
                    ),
                );
                return;
            }
        };
        match image_io::decode(&bytes) {
            Ok(img) => self.state.source = Some(img),
            Err(e) => {
                self.state.source = None;
                self.set_status(cx, &format!("cannot decode {}: {e}", self.state.image_path));
                return;
            }
        }

        if self.state.model.is_none() {
            match HandPipeline::load(&self.state.model_dir) {
                Ok(m) => self.state.model = Some(m),
                Err(e) => {
                    self.present(cx);
                    self.set_status(cx, &format!("showing the image without landmarks: {e}"));
                    return;
                }
            }
        }

        self.run_inference(cx);
    }

    fn run_inference(&mut self, cx: &mut Cx) {
        let (Some(model), Some(img)) = (self.state.model.as_ref(), self.state.source.as_ref())
        else {
            return;
        };

        let started = std::time::Instant::now();
        let result = model.run(&img.rgb, img.width, img.height);
        let elapsed = started.elapsed();

        let summary = match &result {
            Ok(hands) if hands.is_empty() => format!(
                "{}x{} · {:.1} ms · no palm detected",
                img.width,
                img.height,
                elapsed.as_secs_f32() * 1000.0
            ),
            Ok(hands) => {
                let described: Vec<String> = hands
                    .iter()
                    .map(|t| {
                        let side = match t.hand.handedness {
                            Handedness::Left => "left",
                            Handedness::Right => "right",
                        };
                        format!(
                            "{side} {:.0}% (palm {:.2}, presence {:.2})",
                            t.hand.handedness_score * 100.0,
                            t.palm.score,
                            t.hand.presence
                        )
                    })
                    .collect();
                format!(
                    "{}x{} · {:.1} ms · {} hand(s): {}",
                    img.width,
                    img.height,
                    elapsed.as_secs_f32() * 1000.0,
                    hands.len(),
                    described.join(" · ")
                )
            }
            Err(e) => format!("inference failed: {e}"),
        };

        self.state.hands = result.unwrap_or_default();
        self.present(cx);
        self.set_status(cx, &summary);
        self.update_tips(cx);
    }

    fn present(&mut self, cx: &mut Cx) {
        let Some(img) = self.state.source.as_ref() else {
            return;
        };

        let mut rgba = Vec::with_capacity(img.width * img.height * 4);
        for px in img.rgb.chunks_exact(3) {
            rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
        }

        if self.state.show_landmarks {
            let mut canvas = Canvas {
                pixels: &mut rgba,
                width: img.width,
                height: img.height,
            };
            for tracked in self.state.hands.iter() {
                if tracked.hand.presence >= MIN_PRESENCE {
                    draw_hand(&mut canvas, &tracked.hand);
                    draw_handedness(&mut canvas, &tracked.hand);
                }
            }
        }

        let texture = Texture::new_with_format(
            cx,
            TextureFormat::VecBGRAu8_32 {
                data: Some(rgba8_to_bgra_u32(&rgba)),
                width: img.width,
                height: img.height,
                updated: TextureUpdated::Full,
            },
        );
        self.ui
            .image(cx, ids!(hand_image))
            .set_texture(cx, Some(texture));
        self.ui.redraw(cx);
    }

    fn update_tips(&self, cx: &mut Cx) {
        let Some(tracked) = self.state.hands.first() else {
            return;
        };
        let hand = &tracked.hand;
        let parts: Vec<String> = FINGERTIPS
            .iter()
            .map(|i| {
                let lm = hand.landmarks[*i];
                format!("{}: ({:.2}, {:.2}, {:+.2})", LANDMARK_NAMES[*i], lm.x, lm.y, lm.z)
            })
            .collect();
        self.ui
            .label(cx, ids!(tips_label))
            .set_text(cx, &parts.join("   "));
    }

    fn set_status(&mut self, cx: &mut Cx, text: &str) {
        self.ui.label(cx, ids!(status_label)).set_text(cx, text);
    }
}

impl MatchEvent for App {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(cx, ids!(reload_btn)).clicked(actions) {
            self.reload(cx);
        }
        if self.ui.button(cx, ids!(toggle_btn)).clicked(actions) {
            self.state.show_landmarks = !self.state.show_landmarks;
            let label = if self.state.show_landmarks {
                "Hide landmarks"
            } else {
                "Show landmarks"
            };
            self.ui.button(cx, ids!(toggle_btn)).set_text(cx, label);
            self.present(cx);
        }
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        crate::makepad_widgets::script_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());

        if let Event::Startup = event {
            self.state.show_landmarks = true;
            self.reload(cx);
        }
    }
}
