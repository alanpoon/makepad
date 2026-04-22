pub use makepad_widgets;

mod speech_input;

use makepad_widgets::makepad_draw::CxMediaApi;
use makepad_widgets::*;

use speech_input::{
    DrawMicButton, DrawSpinner, SpeechInput, SpeechInputAction,
    process_audio_input,
};

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*

    // Microphone button shader
    set_type_default() do #(DrawMicButton::script_shader(vm)){
        ..mod.draw.DrawQuad
        is_recording: 0.0
        amplitude: 0.0
        accent_color: #FF6600

        pixel: fn() {
            let p = self.pos - vec2(0.5, 0.5)
            let r = length(p)

            // Background circle
            let bg_radius = 0.42
            let bg_mask = clamp(1.0 - (r - bg_radius) * 80.0, 0.0, 1.0)

            // Colors - use accent_color when recording
            let bg_off = vec3(0.25, 0.28, 0.30)
            let bg_on = self.accent_color.xyz
            let bg_color = bg_off.mix(bg_on, self.is_recording)

            // Pulsing glow when recording
            let glow_radius = 0.48 + self.amplitude * 0.08
            let glow_mask = clamp(1.0 - (r - glow_radius) * 20.0, 0.0, 1.0) * self.is_recording * 0.5

            // Amplitude level indicator bars
            let level_y_start = -0.35
            let level_height = 0.05
            let level_spacing = 0.07
            let level_width = 0.25

            let mut in_level = false
            let mut level_color = vec3(0.2, 0.9, 0.5)

            if abs(p.x) < level_width && p.y > level_y_start && p.y < level_y_start + level_height && self.amplitude > 0.2 {
                in_level = true
            }
            if abs(p.x) < level_width && p.y > level_y_start + level_spacing && p.y < level_y_start + level_spacing + level_height && self.amplitude > 0.5 {
                in_level = true
            }
            if abs(p.x) < level_width && p.y > level_y_start + level_spacing * 2.0 && p.y < level_y_start + level_spacing * 2.0 + level_height && self.amplitude > 0.8 {
                in_level = true
                level_color = vec3(0.9, 0.7, 0.2)
            }

            // Microphone icon
            let mic_width = 0.08
            let mic_height = 0.18
            let mic_top = 0.05
            let mic_body = abs(p.x) < mic_width && p.y > -mic_height && p.y < mic_top
            let mic_head = length(p - vec2(0.0, mic_top)) < mic_width
            let stand = abs(p.x) < 0.015 && p.y > -mic_height - 0.06 && p.y < -mic_height + 0.02
            let arc_dist = abs(length(p - vec2(0.0, -0.02)) - 0.12)
            let arc = arc_dist < 0.02 && p.y < -0.02
            let mic_icon = mic_body || mic_head || stand || arc

            // Compose layers
            let mut color = bg_color * 0.6
            let mut alpha = glow_mask
            color = color.mix(bg_color, bg_mask)
            alpha = max(alpha, bg_mask)

            if in_level && bg_mask > 0.5 { color = level_color }
            if mic_icon && bg_mask > 0.5 { color = vec3(1.0, 1.0, 1.0) }

            return vec4(color, alpha)
        }
    }

    // Spinner shader
    set_type_default() do #(DrawSpinner::script_shader(vm)){
        ..mod.draw.DrawQuad
        color: #FF6600
        time: 0.0

        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)

            let stroke_width = 4.0
            let radius = min(self.rect_size.x * 0.5, self.rect_size.y * 0.5) - stroke_width * 0.5
            let center = self.rect_size * 0.5

            let rotation = self.time * 2.0 * PI * 1.2

            let rotation_cycles = rotation / (2.0 * PI)
            let arc_phase = modf(rotation_cycles * 0.5, 1.0)

            let expand_phase = clamp(arc_phase / 0.55, 0.0, 1.0)
            let contract_phase = clamp((arc_phase - 0.55) / 0.45, 0.0, 1.0)

            let cycle = expand_phase * (1.0 - contract_phase)

            let gap_ratio = mix(0.12, 0.92, cycle)
            let gap_radians = gap_ratio * 2.0 * PI

            let start_angle = rotation

            sdf.arc_round_caps(
                center.x
                center.y
                radius
                start_angle
                start_angle + 2.0 * PI - gap_radians
                stroke_width
            )

            return sdf.fill(self.color)
        }
    }

    // Register SpeechInput widget
    mod.widgets.SpeechInputBase = #(SpeechInput::register_widget(vm))
    mod.widgets.SpeechInput = set_type_default() do mod.widgets.SpeechInputBase {
        width: Fill
        height: Fit
        flow: Down
        spacing: 10

        // Default colors
        accent_color: #FF6600

        // Default sizes
        mic_button_size: 40.0

        // Spinner color
        draw_spinner.color: #FF6600

        // Inner TextInput widget
        text_input := TextInput {
            width: Fill
            height: 50
            empty_text: "Type or speak..."
            draw_bg.color: #FFFFFF
            draw_bg.border_color: #FF6600
            draw_bg.border_size: 2.0
            draw_bg.border_radius: 25.0
            padding: {left: 15, right: 55, top: 12, bottom: 12}
            draw_text.text_style.font_size: 14
        }
    }

    let state = {
        status: "Ready - Click microphone to start"
    }
    mod.state = state

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.title: "Speech to Text"
                window.inner_size: vec2(700, 250)
                body +: {
                    main_view := View{
                        width: Fill
                        height: Fill
                        flow: Down
                        spacing: 20
                        padding: 30
                        align: Center
                        draw_bg.color: #5A5A5A

                        // Status label
                        status_label := Label{
                            text: "Ready - Click microphone to start"
                            draw_text.text_style.font_size: 14
                            draw_text.color: #CCCCCC
                        }
                        // The standalone speech input widget
                        speech_input := mod.widgets.SpeechInput{
                            text_input: TextInput{
                                width: Fill
                                height: Fit
                            }
                            // Customization examples:
                            // accent_color: #00AAFF
                            // mic_button_size: 50.0
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
    audio_initialized: bool,
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        cx.use_audio_inputs(&[]);

        // Initialize the speech input widget
        if let Some(mut speech_input) = self.ui.widget(cx, ids!(speech_input)).borrow_mut::<SpeechInput>() {
            speech_input.init(cx);
        }
    }

    fn handle_audio_devices(&mut self, cx: &mut Cx, devices: &AudioDevicesEvent) {
        cx.use_audio_inputs(&devices.default_input());
        self.audio_initialized = true;
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        // Handle speech input actions
        if let Some(speech_input) = self.ui.widget(cx, ids!(speech_input)).borrow::<SpeechInput>() {
            if let Some(action) = speech_input.handle_action(actions) {
                match action {
                    SpeechInputAction::RecordingStarted => {
                        self.ui.label(cx, ids!(status_label)).set_text(cx, "Recording... Click mic to stop");
                    }
                    SpeechInputAction::RecordingStopped => {
                        // Status will be updated when transcription starts/completes
                    }
                    SpeechInputAction::TranscriptionStarted => {
                        self.ui.label(cx, ids!(status_label)).set_text(cx, "Transcribing...");
                    }
                    SpeechInputAction::TranscriptionComplete(_text) => {
                        self.ui.label(cx, ids!(status_label)).set_text(cx, "Ready - Click microphone to continue");
                    }
                    SpeechInputAction::TranscriptionError(err) => {
                        self.ui.label(cx, ids!(status_label)).set_text(cx, &format!("Error: {}", err));
                    }
                    SpeechInputAction::TextChanged(_) => {}
                    SpeechInputAction::None => {}
                }
            }
        }
    }
}

impl App {
    fn start_audio_capture(&mut self, cx: &mut Cx) {
        // Get the recording state from the speech input widget
        let state = if let Some(speech_input) = self.ui.widget(cx, ids!(speech_input)).borrow::<SpeechInput>() {
            speech_input.recording_state()
        } else {
            None
        };

        if let Some(state) = state {
            cx.audio_input(0, move |info, input_buffer| {
                process_audio_input(&state, info, input_buffer);
            });
        }
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        crate::makepad_widgets::script_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        let _ = self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());

        if let Event::AudioDevices(_) = event {
            self.start_audio_capture(cx);
        }
    }
}
