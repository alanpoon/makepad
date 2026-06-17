pub use makepad_widgets;

mod doubao_asr;

use makepad_widgets::makepad_draw::CxMediaApi;
use makepad_widgets::*;
use makepad_widgets::makepad_platform::makepad_network::{NetworkResponse, WsSend, HttpRequest, HttpMethod};

use doubao_asr::{
    DrawMicButton, DrawSpinner, DoubaoAsrInput, DoubaoAsrInputAction,
    DoubaoAsrState, SessionState, process_audio_input,
    build_config_frame, build_audio_frame, new_reqid, parse_response_frame,
};

use std::sync::Arc;

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*

    // Microphone button shader
    set_type_default() do #(DrawMicButton::script_shader(vm)){
        ..mod.draw.DrawQuad
        is_recording: 0.0
        amplitude: 0.0
        accent_color: #00AAFF

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
        color: #00AAFF
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

    // Register DoubaoAsrInput widget
    mod.widgets.DoubaoAsrInputBase = #(DoubaoAsrInput::register_widget(vm))
    mod.widgets.DoubaoAsrInput = set_type_default() do mod.widgets.DoubaoAsrInputBase {
        width: Fill
        height: Fit
        flow: Down
        spacing: 10

        // Default colors
        accent_color: #00AAFF

        // Default sizes
        mic_button_size: 40.0

        // Spinner color
        draw_spinner.color: #00AAFF

        // Inner TextInput widget
        text_input := TextInput {
            width: Fill
            height: 50
            empty_text: "Type or speak Chinese..."
            draw_bg.color: #FFFFFF
            draw_bg.border_color: #00AAFF
            draw_bg.border_size: 2.0
            draw_bg.border_radius: 25.0
            padding: {left: 15, right: 55, top: 12, bottom: 12}
            draw_text.text_style.font_size: 14
        }

        // Interim result label
        interim_label := Label {
            width: Fill
            height: Fit
            draw_text.color: #888888
            draw_text.text_style.font_size: 12
        }
    }

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.title: "Doubao ASR — Speech to Text"
                window.inner_size: vec2(700, 300)
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
                            text: "Ready — click microphone to start"
                            draw_text.text_style.font_size: 14
                            draw_text.color: #CCCCCC
                        }
                        // The Doubao ASR input widget
                        asr_input := mod.widgets.DoubaoAsrInput{
                            text_input: TextInput{
                                width: Fill
                                height: Fit
                            }
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
    ws_id: LiveId,
    #[rust]
    asr_state: Option<Arc<DoubaoAsrState>>,
    #[rust]
    app_id: String,
    #[rust]
    access_token: String,
    #[rust]
    audio_initialized: bool,
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        self.ws_id = live_id!(doubao_asr_socket);
        self.app_id = std::env::var("DOUBAO_APP_ID").unwrap_or_default();
        self.access_token = std::env::var("DOUBAO_ACCESS_TOKEN").unwrap_or_default();

        let state = DoubaoAsrState::new();
        self.asr_state = Some(state.clone());

        if let Some(mut widget) = self.ui.widget(cx, ids!(asr_input)).borrow_mut::<DoubaoAsrInput>() {
            widget.init(cx, state, cx.net.clone(), self.ws_id);
        }

        if self.app_id.is_empty() || self.access_token.is_empty() {
            self.ui.label(cx, ids!(status_label)).set_text(
                cx, "Set DOUBAO_APP_ID and DOUBAO_ACCESS_TOKEN env vars"
            );
        }

        cx.use_audio_inputs(&[]);
    }

    fn handle_audio_devices(&mut self, cx: &mut Cx, devices: &AudioDevicesEvent) {
        cx.use_audio_inputs(&devices.default_input());
        self.audio_initialized = true;
        self.wire_audio_input(cx);
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if let Some(asr_widget) = self.ui.widget(cx, ids!(asr_input)).borrow::<DoubaoAsrInput>() {
            if let Some(action) = asr_widget.handle_action(actions) {
                match action {
                    DoubaoAsrInputAction::RecordingStarted => self.on_recording_started(cx),
                    DoubaoAsrInputAction::RecordingStopped => self.on_recording_stopped(cx),
                    _ => {}
                }
            }
        }
    }
}

impl App {
    fn on_recording_started(&mut self, cx: &mut Cx) {
        if self.app_id.is_empty() || self.access_token.is_empty() { return; }
        let state = match &self.asr_state { Some(s) => s.clone(), None => return };
        let _ = cx.net.ws_close(self.ws_id);
        state.start_recording();
        state.set_session(SessionState::Connecting);

        let url = "wss://openspeech.bytedance.com/api/v2/asr".to_string();
        let mut request = HttpRequest::new(url, HttpMethod::GET);
        request.set_header("X-Api-App-Key".to_string(), self.app_id.clone());
        request.set_header("X-Api-Access-Key".to_string(), self.access_token.clone());
        let _ = cx.net.ws_open(self.ws_id, request);

        self.ui.label(cx, ids!(status_label)).set_text(cx, "Connecting\u{2026}");
    }

    fn on_recording_stopped(&mut self, cx: &mut Cx) {
        let state = match &self.asr_state { Some(s) => s.clone(), None => return };
        state.stop_recording();
        let eos = build_audio_frame(&[], true);
        let _ = cx.net.ws_send(self.ws_id, WsSend::Binary(eos));
        state.set_session(SessionState::Closing);
        self.ui.label(cx, ids!(status_label)).set_text(cx, "Waiting for final result\u{2026}");
    }

    fn wire_audio_input(&mut self, cx: &mut Cx) {
        let state = match &self.asr_state { Some(s) => s.clone(), None => return };
        cx.audio_input(0, move |info, buf| {
            process_audio_input(&state, info, buf);
        });
    }

    fn handle_network_responses(&mut self, cx: &mut Cx, responses: &[NetworkResponse]) {
        for response in responses {
            match response {
                NetworkResponse::WsOpened { socket_id } if *socket_id == self.ws_id => {
                    self.on_ws_opened(cx);
                }
                NetworkResponse::WsMessage { socket_id, message } if *socket_id == self.ws_id => {
                    if let makepad_widgets::makepad_platform::makepad_network::WsMessage::Binary(data) = message {
                        let data = data.clone();
                        self.on_ws_message(cx, &data);
                    }
                }
                NetworkResponse::WsError { socket_id, message } if *socket_id == self.ws_id => {
                    let msg = message.clone();
                    self.on_ws_error(cx, msg);
                }
                NetworkResponse::WsClosed { socket_id } if *socket_id == self.ws_id => {
                    self.on_ws_closed(cx);
                }
                _ => {}
            }
        }
    }

    fn on_ws_opened(&mut self, cx: &mut Cx) {
        let state = match &self.asr_state { Some(s) => s.clone(), None => return };
        let reqid = new_reqid();
        let frame = build_config_frame(&self.app_id, &self.access_token, &reqid);
        let _ = cx.net.ws_send(self.ws_id, WsSend::Binary(frame));
        state.set_session(SessionState::Streaming);
        self.ui.label(cx, ids!(status_label)).set_text(cx, "Recording \u{2014} click mic to stop");
    }

    fn on_ws_message(&mut self, cx: &mut Cx, data: &[u8]) {
        let state = match &self.asr_state { Some(s) => s.clone(), None => return };
        let Some(resp) = parse_response_frame(data) else {
            log!("doubao_asr: failed to parse response frame");
            return;
        };
        let code = resp.code.unwrap_or(1000);
        if code != 1000 {
            let msg = resp.message.unwrap_or_else(|| format!("ASR error code {code}"));
            state.set_session(SessionState::Error(msg.clone()));
            self.ui.label(cx, ids!(status_label)).set_text(cx, &format!("Error: {msg}"));
            return;
        }
        if let Some(result) = resp.result {
            let text = result.text.unwrap_or_default();
            let is_final = result.is_final.unwrap_or(false);
            if is_final {
                let existing = self.ui.text_input(cx, ids!(asr_input.text_input)).text();
                let new_text = format!("{}{}", existing, text);
                self.ui.text_input(cx, ids!(asr_input.text_input)).set_text(cx, &new_text);
                *state.interim_text.lock().unwrap() = String::new();
                *state.confirmed_text.lock().unwrap() = new_text;
                if matches!(*state.session.lock().unwrap(), SessionState::Closing) {
                    state.set_session(SessionState::Idle);
                    use std::sync::atomic::Ordering;
                    state.is_recording.store(false, Ordering::SeqCst);
                    self.ui.label(cx, ids!(status_label)).set_text(cx, "Ready \u{2014} click microphone to start");
                }
            } else {
                *state.interim_text.lock().unwrap() = text;
            }
        }
    }

    fn on_ws_error(&mut self, cx: &mut Cx, message: String) {
        let state = match &self.asr_state { Some(s) => s.clone(), None => return };
        state.stop_recording();
        state.pending_samples.lock().unwrap().clear();
        state.set_session(SessionState::Error(message.clone()));
        self.ui.label(cx, ids!(status_label)).set_text(cx, &format!("Error: {message}"));
    }

    fn on_ws_closed(&mut self, cx: &mut Cx) {
        let state = match &self.asr_state { Some(s) => s.clone(), None => return };
        let session = std::mem::replace(&mut *state.session.lock().unwrap(), SessionState::Idle);
        match session {
            SessionState::Closing => {
                self.ui.label(cx, ids!(status_label)).set_text(cx, "Ready \u{2014} click microphone to start");
            }
            SessionState::Streaming => {
                let msg = "Session closed unexpectedly".to_string();
                *state.session.lock().unwrap() = SessionState::Error(msg.clone());
                state.pending_samples.lock().unwrap().clear();
                state.stop_recording();
                self.ui.label(cx, ids!(status_label)).set_text(cx, &format!("Error: {msg}"));
            }
            _ => {}
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

        if let Event::NetworkResponses(responses) = event {
            let responses = responses.clone();
            self.handle_network_responses(cx, &responses);
        }
    }
}
