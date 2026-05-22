pub use makepad_widgets;

pub mod decoder;
pub mod player;

use std::sync::{Arc, Mutex};

use decoder::{decode_audio, DecodedPcm};
use makepad_widgets::*;
use player::{mix_audio_output, PlayerState};

app_main!(App);

const SAMPLE_MP3: &[u8] = include_bytes!("../resources/sample.mp3");
const SAMPLE_WAV: &[u8] = include_bytes!("../resources/sample.wav");
const SAMPLE_AIFF: &[u8] = include_bytes!("../resources/sample.aiff");
const SAMPLE_FLAC: &[u8] = include_bytes!("../resources/sample.flac");
const SAMPLE_M4A: &[u8] = include_bytes!("../resources/sample.m4a");

const FORMATS: &[(&str, &str, &[u8])] = &[
    ("MP3", "mp3", SAMPLE_MP3),
    ("WAV", "wav", SAMPLE_WAV),
    ("AIFF", "aiff", SAMPLE_AIFF),
    ("FLAC", "flac", SAMPLE_FLAC),
    ("ALAC", "m4a", SAMPLE_M4A),
];

script_mod! {
    use mod.prelude.widgets.*
    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.inner_size: vec2(420, 620)
                body +: {
                    View{
                        width: Fill
                        height: Fill
                        flow: Down
                        spacing: 10
                        align: Center
                        padding: 16
                        Label{
                            text: "Audio Format Tester"
                            draw_text.text_style.font_size: 18
                        }
                        View{
                            width: Fill
                            height: Fit
                            flow: Right
                            spacing: 12
                            align: Align{x: 0.0 y: 0.5}
                            Label{ text: "MP3" width: 60 }
                            play_mp3 := Button{ text: "Play" }
                        }
                        View{
                            width: Fill
                            height: Fit
                            flow: Right
                            spacing: 12
                            align: Align{x: 0.0 y: 0.5}
                            Label{ text: "WAV" width: 60 }
                            play_wav := Button{ text: "Play" }
                        }
                        View{
                            width: Fill
                            height: Fit
                            flow: Right
                            spacing: 12
                            align: Align{x: 0.0 y: 0.5}
                            Label{ text: "AIFF" width: 60 }
                            play_aiff := Button{ text: "Play" }
                        }
                        View{
                            width: Fill
                            height: Fit
                            flow: Right
                            spacing: 12
                            align: Align{x: 0.0 y: 0.5}
                            Label{ text: "FLAC" width: 60 }
                            play_flac := Button{ text: "Play" }
                        }
                        View{
                            width: Fill
                            height: Fit
                            flow: Right
                            spacing: 12
                            align: Align{x: 0.0 y: 0.5}
                            Label{ text: "ALAC" width: 60 }
                            play_alac := Button{ text: "Play" }
                        }
                        camera_texture_host := View{
                            width: Fill
                            height: 500
                            flow: Down
                            spacing: 8
                            align: Center
                            View{
                                width: Fill
                                height: 500
                                flow: Overlay
                                camera_video_texture := Video{
                                    width: Fill
                                    height: 300
                                    source: VideoDataSource.Dependency {
                                        res: crate_resource("self:resources/confetti_feature_animation.mp4")
                                    }
                                    autoplay: false
                                    is_looping: true
                                    show_controls: true
                                }
                                View{
                                    width: Fill
                                    height: Fill
                                    align: Align{x: 1.0 y: 0.0}
                                    padding: 8
                                    spacing: 8
                                    flow: Right
                                    toggle_thumbnail_btn := Button{ text: "Hide Thumb" }
                                    maximize_top_btn := Button{ text: "⛶" }
                                }
                            }
                            // View{
                            //     width: Fit
                            //     height: Fit
                            //     flow: Right
                            //     spacing: 12
                            //     align: Center
                            //     playpause_main_btn := Button{ text: "Play" }
                            //     maximize_btn := Button{ text: "Maximize" }
                            // }
                        }
                    }
                    video_modal := Modal{
                        content +: {
                            width: Fill
                            height: Fill
                            padding: 16
                            spacing: 12
                            align: Center
                            View{
                                width: Fill
                                height: Fill
                                show_bg: true
                                draw_bg.color: #222
                                padding: 16
                                spacing: 12
                                flow: Down
                                align: Center
                                modal_video := Video{
                                    width: Fill
                                    height: Fill
                                    source: VideoDataSource.Dependency {
                                        res: crate_resource("self:resources/confetti_feature_animation.mp4")
                                    }
                                    autoplay: false
                                    is_looping: true
                                    show_controls: true
                                }
                                View{
                                    width: Fit
                                    height: Fit
                                    flow: Right
                                    spacing: 12
                                    align: Center
                                    playpause_modal_btn := Button{ text: "Pause" }
                                    close_modal_btn := Button{ text: "Close" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

struct Track {
    source: Option<Arc<DecodedPcm>>,
    state: Arc<Mutex<PlayerState>>,
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
    #[rust]
    tracks: Vec<Track>,
    #[rust]
    pending_fullscreen: Option<NextFrame>,
    #[rust]
    pending_normalize: Option<NextFrame>,
    #[rust]
    pending_modal_seek_ms: Option<u64>,
    #[rust(true)]
    thumbnail_visible: bool,
}

impl App {
    fn toggle(&mut self, cx: &mut Cx, index: usize, button_path: &[LiveId]) {
        let Some(track) = self.tracks.get(index) else { return };
        if track.source.is_none() {
            return;
        }
        let playing = if let Ok(mut state) = track.state.lock() {
            state.playing = !state.playing;
            state.playing
        } else {
            false
        };
        self.ui
            .button(cx, button_path)
            .set_text(cx, if playing { "Pause" } else { "Play" });
    }

    fn close_video_modal(&mut self, cx: &mut Cx) {
        self.ui
            .video(cx, ids!(modal_video))
            .stop_and_cleanup_resources(cx);
        self.ui
            .video(cx, ids!(camera_video_texture))
            .begin_playback(cx);
        self.ui
            .button(cx, ids!(playpause_main_btn))
            .set_text(cx, "Pause");
        self.pending_normalize = Some(cx.new_next_frame());
        self.ui.modal(cx, ids!(video_modal)).close(cx);
    }
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        self.tracks = FORMATS
            .iter()
            .map(|(label, ext, bytes)| {
                let source = match decode_audio(bytes, ext) {
                    Ok(decoded) => Some(Arc::new(decoded)),
                    Err(err) => {
                        error!("media_player: {label} decode failed: {err}");
                        None
                    }
                };
                Track {
                    source,
                    state: Arc::new(Mutex::new(PlayerState::default())),
                }
            })
            .collect();

        let callbacks: Vec<(Arc<DecodedPcm>, Arc<Mutex<PlayerState>>)> = self
            .tracks
            .iter()
            .filter_map(|track| {
                track
                    .source
                    .as_ref()
                    .map(|source| (source.clone(), track.state.clone()))
            })
            .collect();

        cx.audio_output(0, move |info, output| {
            output.zero();
            for (source, state) in callbacks.iter() {
                if let Ok(mut state) = state.lock() {
                    mix_audio_output(&mut state, source, info, output);
                }
            }
        });

        // Build a solid grey BGRA texture and show it in place of the video frame.
        // Layout: little-endian u32 = 0xAA_RR_GG_BB after byte swap → bytes B,G,R,A.
        let grey: u32 = 0xFF80_8080;
        let width = 64usize;
        let height = 64usize;
        let grey_tex = Texture::new_with_format(
            cx,
            TextureFormat::VecBGRAu8_32 {
                data: Some(vec![grey; width * height]),
                width,
                height,
                updated: TextureUpdated::Full,
            },
        );
        let video = self.ui.video(cx, ids!(camera_video_texture));
        video.set_thumbnail_texture(cx, Some(grey_tex));
        video.show_thumbnail(cx, true);
    }

    fn handle_audio_devices(&mut self, cx: &mut Cx, devices: &AudioDevicesEvent) {
        cx.use_audio_outputs(&devices.default_output());
    }

    fn handle_next_frame(&mut self, cx: &mut Cx, e: &NextFrameEvent) {
        if let Some(nf) = self.pending_fullscreen {
            if e.set.contains(&nf) {
                log!("[lib] deferred fullscreen window");
                self.ui.window(cx, ids!(main_window)).fullscreen(cx);
                self.pending_fullscreen = None;
            }
        }
        if let Some(nf) = self.pending_normalize {
            if e.set.contains(&nf) {
                log!("[lib] deferred normalize window");
                self.ui.window(cx, ids!(main_window)).disable_fullscreen(cx);
                self.pending_normalize = None;
            }
        }
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(cx, ids!(toggle_thumbnail_btn)).clicked(actions) {
            self.thumbnail_visible = !self.thumbnail_visible;
            self.ui
                .video(cx, ids!(camera_video_texture))
                .show_thumbnail(cx, self.thumbnail_visible);
            self.ui.button(cx, ids!(toggle_thumbnail_btn)).set_text(
                cx,
                if self.thumbnail_visible { "Hide Thumb" } else { "Show Thumb" },
            );
        }
        return;
        if self.ui.button(cx, ids!(play_mp3)).clicked(actions) {
            self.toggle(cx, 0, ids!(play_mp3));
        }
        if self.ui.button(cx, ids!(play_wav)).clicked(actions) {
            self.toggle(cx, 1, ids!(play_wav));
        }
        if self.ui.button(cx, ids!(play_aiff)).clicked(actions) {
            self.toggle(cx, 2, ids!(play_aiff));
        }
        if self.ui.button(cx, ids!(play_flac)).clicked(actions) {
            self.toggle(cx, 3, ids!(play_flac));
        }
        if self.ui.button(cx, ids!(play_alac)).clicked(actions) {
            self.toggle(cx, 4, ids!(play_alac));
        }
        if self.ui.button(cx, ids!(playpause_main_btn)).clicked(actions) {
            let video = self.ui.video(cx, ids!(camera_video_texture));
            if video.is_playing() {
                log!("[lib] playpause_main_btn: pausing");
                video.pause_playback(cx);
                self.ui
                    .button(cx, ids!(playpause_main_btn))
                    .set_text(cx, "Play");
            } else if video.is_paused() {
                log!("[lib] playpause_main_btn: resuming");
                video.resume_playback(cx);
                self.ui
                    .button(cx, ids!(playpause_main_btn))
                    .set_text(cx, "Pause");
            } else {
                log!("[lib] playpause_main_btn: begin playback");
                video.begin_playback(cx);
                self.ui
                    .button(cx, ids!(playpause_main_btn))
                    .set_text(cx, "Pause");
            }
        }
        let maximize_clicked = self.ui.button(cx, ids!(maximize_btn)).clicked(actions)
            || self.ui.button(cx, ids!(maximize_top_btn)).clicked(actions);
        if maximize_clicked {
            let main_pos_ms = self
                .ui
                .video(cx, ids!(camera_video_texture))
                .current_position_ms() as u64;
            self.pending_modal_seek_ms = Some(main_pos_ms);
            log!(
                "[lib] maximize clicked: stop main at {} ms, open modal, begin modal_video, fullscreen window",
                main_pos_ms,
            );
            self.ui
                .video(cx, ids!(camera_video_texture))
                .stop_and_cleanup_resources(cx);
            self.ui
                .button(cx, ids!(playpause_main_btn))
                .set_text(cx, "Play");
            self.ui.modal(cx, ids!(video_modal)).open(cx);
            self.ui
                .video(cx, ids!(modal_video))
                .begin_playback(cx);
            self.ui
                .button(cx, ids!(playpause_modal_btn))
                .set_text(cx, "Pause");
            self.pending_fullscreen = Some(cx.new_next_frame());
        }
        let modal_video_ref = self.ui.video(cx, ids!(modal_video));
        if matches!(
            actions
                .find_widget_action(modal_video_ref.widget_uid())
                .cast::<VideoAction>(),
            VideoAction::PlaybackPrepared
        ) {
            if let Some(ms) = self.pending_modal_seek_ms.take() {
                log!("[lib] modal_video prepared, seeking to {} ms", ms);
                modal_video_ref.seek_to(cx, ms);
            }
        }
        if self.ui.button(cx, ids!(playpause_modal_btn)).clicked(actions) {
            let video = self.ui.video(cx, ids!(modal_video));
            if video.is_playing() {
                log!("[lib] playpause_modal_btn: pausing");
                video.pause_playback(cx);
                self.ui
                    .button(cx, ids!(playpause_modal_btn))
                    .set_text(cx, "Play");
            } else if video.is_paused() {
                log!("[lib] playpause_modal_btn: resuming");
                video.resume_playback(cx);
                self.ui
                    .button(cx, ids!(playpause_modal_btn))
                    .set_text(cx, "Pause");
            } else {
                log!("[lib] playpause_modal_btn: begin playback");
                video.begin_playback(cx);
                self.ui
                    .button(cx, ids!(playpause_modal_btn))
                    .set_text(cx, "Pause");
            }
        }
        let close_clicked = self.ui.button(cx, ids!(close_modal_btn)).clicked(actions);
        let dismissed = self.ui.modal(cx, ids!(video_modal)).dismissed(actions);
        if close_clicked || dismissed {
            self.close_video_modal(cx);
        }
    }

    fn handle_key_down(&mut self, cx: &mut Cx, e: &KeyEvent) {
        if e.key_code == KeyCode::Escape
            && self.ui.modal(cx, ids!(video_modal)).is_open()
        {
            log!("[lib] Escape pressed, closing modal");
            self.close_video_modal(cx);
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

    #[test]
    fn test_format_table_lists_all_five_codecs() {
        let labels: Vec<&str> = FORMATS.iter().map(|(label, _, _)| *label).collect();
        assert_eq!(labels, vec!["MP3", "WAV", "AIFF", "FLAC", "ALAC"]);
    }
}
