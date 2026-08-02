//! Loads an image, runs MoveNet over it, and shows the skeleton.
//!
//! Paths come from the command line:
//!   cargo run -p makepad-example-pose-movenet -- <image> [model-dir]
//! and default to `./pose.jpg` and `./model`.

use makepad_widgets::*;

use crate::image_io;
use crate::movenet::{Estimator, Pose, KEYPOINT_NAMES};
use crate::overlay::{draw_pose, rgba8_to_bgra_u32, Canvas};

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
                            text: "MoveNet single pose"
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
                            toggle_btn := Button{ text: "Hide skeleton" }
                        }

                        image_host := View{
                            width: Fill
                            height: Fill
                            align: Center
                            pose_image := Image{
                                width: Fill
                                height: Fill
                                fit: ImageFit.Smallest
                            }
                        }

                        scores_label := Label{
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
    estimator: Option<Estimator>,
    source: Option<image_io::DecodedImage>,
    pose: Option<Pose>,
    show_skeleton: bool,
    status: String,
}

/// Joints below this confidence are left off the drawing.
const MIN_KEYPOINT_SCORE: f32 = 0.25;

impl App {
    fn args() -> (String, String) {
        let mut args = std::env::args().skip(1).filter(|a| !a.starts_with('-'));
        let image = args.next().unwrap_or_else(|| "pose.jpg".to_string());
        let model = args.next().unwrap_or_else(|| "model".to_string());
        (image, model)
    }

    fn reload(&mut self, cx: &mut Cx) {
        let (image_path, model_dir) = Self::args();
        self.state.image_path = image_path;
        self.state.model_dir = model_dir;
        self.state.pose = None;

        // 1. the picture
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

        // 2. the model, which is optional: without it we still show the image
        if self.state.estimator.is_none() {
            match Estimator::load(&self.state.model_dir) {
                Ok(est) => self.state.estimator = Some(est),
                Err(e) => {
                    self.present(cx);
                    self.set_status(
                        cx,
                        &format!(
                            "showing {} without a pose: {e}\n\
                             run tools/convert_movenet.py to build {}/movenet.json \
                             and {}/movenet.safetensors",
                            self.state.image_path, self.state.model_dir, self.state.model_dir
                        ),
                    );
                    return;
                }
            }
        }

        self.run_inference(cx);
    }

    fn run_inference(&mut self, cx: &mut Cx) {
        let (Some(est), Some(img)) = (self.state.estimator.as_ref(), self.state.source.as_ref())
        else {
            return;
        };

        let started = std::time::Instant::now();
        let result = est.estimate(&img.rgb, img.width, img.height);
        let elapsed = started.elapsed();
        // summarize before the borrows end so present() can take &mut self
        let summary = match &result {
            Ok(pose) => {
                let visible = pose
                    .keypoints
                    .iter()
                    .filter(|k| k.score >= MIN_KEYPOINT_SCORE)
                    .count();
                format!(
                    "{} · {}x{} · {:.1} ms · {visible}/17 joints above {MIN_KEYPOINT_SCORE}",
                    est.name(),
                    img.width,
                    img.height,
                    elapsed.as_secs_f32() * 1000.0
                )
            }
            Err(e) => format!("inference failed: {e}"),
        };

        self.state.pose = result.ok();
        self.present(cx);
        self.set_status(cx, &summary);
        self.update_scores(cx);
    }

    /// Compose image + skeleton into one texture and hand it to the widget.
    fn present(&mut self, cx: &mut Cx) {
        let Some(img) = self.state.source.as_ref() else {
            return;
        };

        let mut rgba = Vec::with_capacity(img.width * img.height * 4);
        for px in img.rgb.chunks_exact(3) {
            rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
        }

        if self.state.show_skeleton {
            if let Some(pose) = self.state.pose.as_ref() {
                let mut canvas = Canvas {
                    pixels: &mut rgba,
                    width: img.width,
                    height: img.height,
                };
                draw_pose(&mut canvas, pose, MIN_KEYPOINT_SCORE);
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
            .image(cx, ids!(pose_image))
            .set_texture(cx, Some(texture));
        self.ui.redraw(cx);
    }

    fn update_scores(&self, cx: &mut Cx) {
        let Some(pose) = self.state.pose.as_ref() else {
            return;
        };
        let mut parts: Vec<String> = Vec::new();
        for (i, kp) in pose.keypoints.iter().enumerate() {
            parts.push(format!("{}: {:.2}", KEYPOINT_NAMES[i], kp.score));
        }
        self.ui
            .label(cx, ids!(scores_label))
            .set_text(cx, &parts.join("   "));
    }

    fn set_status(&mut self, cx: &mut Cx, text: &str) {
        self.state.status = text.to_string();
        self.ui.label(cx, ids!(status_label)).set_text(cx, text);
    }
}

impl MatchEvent for App {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(cx, ids!(reload_btn)).clicked(actions) {
            self.reload(cx);
        }
        if self.ui.button(cx, ids!(toggle_btn)).clicked(actions) {
            self.state.show_skeleton = !self.state.show_skeleton;
            let label = if self.state.show_skeleton {
                "Hide skeleton"
            } else {
                "Show skeleton"
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
            self.state.show_skeleton = true;
            self.reload(cx);
        }
    }
}
