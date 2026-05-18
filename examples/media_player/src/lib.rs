pub use makepad_widgets;

pub mod decoder;
pub mod player;

use std::sync::{Arc, Mutex};

use decoder::{decode_mp3, DecodedPcm};
use makepad_widgets::*;
use player::{fill_audio_output, PlayerState};

app_main!(App);

const SAMPLE_MP3: &[u8] = include_bytes!("../resources/sample.mp3");

script_mod! {
    use mod.prelude.widgets.*
    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.inner_size: vec2(360, 180)
                body +: {
                    View{
                        width: Fill
                        height: Fill
                        flow: Down
                        spacing: 12
                        align: Center
                        Label{
                            text: "Bundled MP3"
                            draw_text.text_style.font_size: 18
                        }
                        play_button := Button{
                            text: "Play"
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
    source: Option<Arc<DecodedPcm>>,
    #[rust]
    state: Arc<Mutex<PlayerState>>,
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        match decode_mp3(SAMPLE_MP3) {
            Ok(source) => {
                let source = Arc::new(source);
                let audio_source = source.clone();
                let audio_state = self.state.clone();
                cx.audio_output(0, move |info, output| {
                    if let Ok(mut state) = audio_state.lock() {
                        fill_audio_output(&mut state, &audio_source, info, output);
                    } else {
                        output.zero();
                    }
                });
                self.source = Some(source);
            }
            Err(err) => {
                error!("media_player: {err}");
            }
        }
    }

    fn handle_audio_devices(&mut self, cx: &mut Cx, devices: &AudioDevicesEvent) {
        cx.use_audio_outputs(&devices.default_output());
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(cx, ids!(play_button)).clicked(actions) {
            let playing = if let Ok(mut state) = self.state.lock() {
                state.playing = !state.playing;
                state.playing
            } else {
                false
            };
            self.ui
                .button(cx, ids!(play_button))
                .set_text(cx, if playing { "Pause" } else { "Play" });
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_crate_smoke_links_app() {
        let _app_type = std::any::type_name::<App>();
        let state = PlayerState::default();
        assert!(!state.playing);
    }
}
