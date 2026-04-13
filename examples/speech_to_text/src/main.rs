pub use makepad_widgets;

use makepad_widgets::makepad_platform::audio::AudioBuffer;
use makepad_widgets::makepad_draw::CxMediaApi;
use makepad_widgets::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const WHISPER_SAMPLE_RATE: f64 = 16000.0;

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*

    // Microphone button with recording indicator
    set_type_default() do #(DrawMicButton::script_shader(vm)){
        ..mod.draw.DrawQuad
        is_recording: 0.0
        amplitude: 0.0

        pixel: fn() {
            let p = self.pos - vec2(0.5, 0.5)
            let r = length(p)

            // Background circle
            let bg_radius = 0.42
            let bg_mask = clamp(1.0 - (r - bg_radius) * 80.0, 0.0, 1.0)

            // Colors
            let bg_off = vec3(0.25, 0.28, 0.30)
            let bg_on = vec3(0.85, 0.20, 0.25)
            let bg_color = bg_off.mix(bg_on, self.is_recording)

            // Pulsing glow when recording
            let glow_radius = 0.48 + self.amplitude * 0.08
            let glow_mask = clamp(1.0 - (r - glow_radius) * 20.0, 0.0, 1.0) * self.is_recording * 0.5

            // Amplitude level indicator (3 horizontal bars at bottom of circle)
            let level_y_start = -0.35
            let level_height = 0.05
            let level_spacing = 0.07
            let level_width = 0.25

            let mut in_level = false
            let mut level_color = vec3(0.2, 0.9, 0.5)  // Green

            // Bar 1 (always shows if amplitude > 0.2)
            let bar1_y = level_y_start
            if abs(p.x) < level_width && p.y > bar1_y && p.y < bar1_y + level_height && self.amplitude > 0.2 {
                in_level = true
            }

            // Bar 2 (shows if amplitude > 0.5)
            let bar2_y = level_y_start + level_spacing
            if abs(p.x) < level_width && p.y > bar2_y && p.y < bar2_y + level_height && self.amplitude > 0.5 {
                in_level = true
            }

            // Bar 3 (shows if amplitude > 0.8)
            let bar3_y = level_y_start + level_spacing * 2.0
            if abs(p.x) < level_width && p.y > bar3_y && p.y < bar3_y + level_height && self.amplitude > 0.8 {
                in_level = true
                level_color = vec3(0.9, 0.7, 0.2)  // Yellow/orange for high level
            }

            // Microphone icon (simplified)
            let mic_width = 0.08
            let mic_height = 0.18
            let mic_top = 0.05

            // Mic body (rectangle with rounded top)
            let mic_body = abs(p.x) < mic_width && p.y > -mic_height && p.y < mic_top
            let mic_head = length(p - vec2(0.0, mic_top)) < mic_width
            let mic_shape = mic_body || mic_head

            // Mic stand
            let stand_width = 0.015
            let stand_y = -mic_height - 0.06
            let stand = abs(p.x) < stand_width && p.y > stand_y && p.y < -mic_height + 0.02

            // Mic arc
            let arc_radius = 0.12
            let arc_center = vec2(0.0, -0.02)
            let arc_dist = abs(length(p - arc_center) - arc_radius)
            let arc = arc_dist < 0.02 && p.y < -0.02

            let mic_icon = mic_shape || stand || arc
            let icon_color = vec3(1.0, 1.0, 1.0)

            // Compose
            let mut color = vec3(0.0, 0.0, 0.0)
            let mut alpha = 0.0

            // Glow layer
            color = bg_color * 0.6
            alpha = glow_mask

            // Background layer
            color = color.mix(bg_color, bg_mask)
            alpha = max(alpha, bg_mask)

            // Level indicator bars (drawn before icon)
            if in_level && bg_mask > 0.5 {
                color = level_color
            }

            // Icon layer (drawn last, on top)
            if mic_icon && bg_mask > 0.5 {
                color = icon_color
            }

            return vec4(color, alpha)
        }
    }

    // Waveform visualizer
    set_type_default() do #(DrawWaveform::script_shader(vm)){
        ..mod.draw.DrawQuad
        amplitude: 0.1
        is_recording: 0.0

        pixel: fn() {
            // SIMPLIFIED TEST: Just draw solid red to verify rendering
            return vec4(1.0, 0.0, 0.0, 1.0)
        }
    }

    // Register widgets
    mod.widgets.MicButtonBase = #(MicButton::register_widget(vm))
    mod.widgets.MicButton = set_type_default() do mod.widgets.MicButtonBase {
        width: 40
        height: 40
        draw_bg.is_recording: 0.0
        draw_bg.amplitude: 0.0
    }

    mod.widgets.WaveformBase = #(Waveform::register_widget(vm))
    mod.widgets.Waveform = set_type_default() do mod.widgets.WaveformBase {
        width: Fill
        height: 60
        draw_bg.amplitude: 0.5
        draw_bg.is_recording: 0.0
    }

    let state = {
        is_recording: false,
        status: "Ready - Click microphone to start"
    }
    mod.state = state

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.title: "Makepad"
                window.inner_size: vec2(700, 200)
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
                            draw_text.color: #999999
                        }

                        // Main input row with rounded border
                        input_row := RoundedView{
                            width: Fill
                            height: 50
                            flow: Right
                            align: Center
                            padding: 8
                            spacing: 10
                            draw_bg.color: #FFFFFF
                            draw_bg.border_color: #FF6600
                            draw_bg.border_size: 2.0
                            draw_bg.border_radius: 25.0

                            // Plus button (left side)
                            View{
                                width: 30
                                height: Fill
                                align: Center
                                plus_button := Label{
                                    text: "+"
                                    draw_text.color: #888888
                                    draw_text.text_style.font_size: 24
                                }
                            }

                            // Duration label (inside input box, only visible when recording)
                            duration_label := Label{
                                text: ""
                                draw_text.color: #E53935
                                draw_text.text_style.font_size: 12
                            }

                            // Text input (center)
                            result_input := TextInput{
                                empty_text: "Type or speak..."
                                width: Fill
                                height: Fit
                                draw_text.text_style.font_size: 14
                            }

                            // Loading spinner (only visible when transcribing)
                            loading_spinner := LoadingSpinner{
                                visible: false
                                width: 20
                                height: 20
                                draw_bg.color: #FF6600
                                draw_bg.rotation_speed: 1.5
                                draw_bg.stroke_width: 2.5
                            }

                            // Custom microphone button with amplitude visualization
                            mic_button := mod.widgets.MicButton{
                                width: 40
                                height: 40
                            }

                            // Send button (black circle with arrow)
                            send_button := RoundedView{
                                width: 36
                                height: 36
                                align: Center
                                draw_bg.color: #222222
                                draw_bg.border_radius: 18.0
                                Label{
                                    text: "↑"
                                    draw_text.color: #FFFFFF
                                    draw_text.text_style.font_size: 18
                                }
                            }
                        }

                        // Waveform visualizer (below input box) - using actual rectangles
                        waveform_container := RoundedView{
                            width: Fill
                            height: 60
                            flow: Right
                            align: Center
                            spacing: 4
                            new_batch: true
                            show_bg: true
                            draw_bg.color: #1a1c1f
                            draw_bg.border_size: 1.0
                            draw_bg.border_color: #404040
                            draw_bg.border_radius: 8.0
                            padding: 12

                            // 30 bars for waveform - green color
                            bar_0 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_1 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_2 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_3 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_4 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_5 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_6 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_7 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_8 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_9 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_10 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_11 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_12 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_13 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_14 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_15 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_16 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_17 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_18 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_19 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_20 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_21 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_22 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_23 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_24 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_25 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_26 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_27 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_28 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                            bar_29 := SolidView{ width: 8, height: 30, draw_bg.color: #20E050, visible: false }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawMicButton {
    #[deref]
    draw_super: DrawQuad,
    #[live]
    is_recording: f32,
    #[live]
    amplitude: f32,
}

#[derive(Script, ScriptHook, Widget)]
pub struct MicButton {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[redraw]
    #[live]
    draw_bg: DrawMicButton,
    #[live(true)]
    #[visible]
    visible: bool,
}

impl Widget for MicButton {
    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.visible {
            return DrawStep::done();
        }
        self.draw_bg.draw_walk(cx, walk);
        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if !self.visible {
            return;
        }
        let uid = self.widget_uid();
        match event.hits(cx, self.draw_bg.area()) {
            Hit::FingerDown(_) => {
                cx.widget_action(uid, MicButtonAction::Clicked);
            }
            _ => {}
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum MicButtonAction {
    #[default]
    None,
    Clicked,
}

impl MicButton {
    pub fn set_recording(&mut self, cx: &mut Cx, is_recording: bool, amplitude: f32) {
        self.draw_bg.is_recording = if is_recording { 1.0 } else { 0.0 };
        self.draw_bg.amplitude = amplitude;
        self.redraw(cx);
    }

    pub fn clicked(&self, actions: &Actions) -> bool {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            if let MicButtonAction::Clicked = item.cast() {
                return true;
            }
        }
        false
    }
}

impl WidgetMatchEvent for MicButton {
    fn handle_actions(&mut self, _cx: &mut Cx, _actions: &Actions, _scope: &mut Scope) {}
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawWaveform {
    #[deref]
    draw_super: DrawQuad,
    #[live]
    amplitude: f32,
    #[live]
    is_recording: f32,
}

#[derive(Script, ScriptHook, Widget)]
pub struct Waveform {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,
    #[redraw]
    #[live]
    draw_bg: DrawWaveform,
    #[live(true)]
    #[visible]
    visible: bool,
}

impl Widget for Waveform {
    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        println!("[DEBUG] Waveform draw_walk called - amplitude={:.3}, is_recording={:.1}, visible={}",
                 self.draw_bg.amplitude, self.draw_bg.is_recording, self.visible);

        if !self.visible {
            return DrawStep::done();
        }

        let rect = cx.walk_turtle(walk);
        self.draw_bg.draw_abs(cx, rect);
        DrawStep::done()
    }
}

impl Waveform {
    pub fn update(&mut self, cx: &mut Cx, amplitude: f32, is_recording: bool) {
        self.draw_bg.amplitude = amplitude;
        self.draw_bg.is_recording = if is_recording { 1.0 } else { 0.0 };
        println!("[DEBUG] Waveform update: amplitude={:.3}, is_recording={}", amplitude, is_recording);
        self.redraw(cx);
    }
}

impl WidgetMatchEvent for Waveform {
    fn handle_actions(&mut self, _cx: &mut Cx, _actions: &Actions, _scope: &mut Scope) {}
}

struct RecordingState {
    is_recording: AtomicBool,
    accumulated_samples: Mutex<Vec<f32>>,
    sample_rate: Mutex<f64>,
    transcription_result: Mutex<Option<String>>,
    recent_samples: Mutex<Vec<f32>>,
}

impl RecordingState {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            is_recording: AtomicBool::new(false),
            accumulated_samples: Mutex::new(Vec::new()),
            sample_rate: Mutex::new(44100.0),
            transcription_result: Mutex::new(None),
            recent_samples: Mutex::new(Vec::new()),
        })
    }
}

fn resample_to_16k_mono(input: &AudioBuffer, from_rate: f64) -> Vec<f32> {
    if input.frame_count() == 0 {
        return Vec::new();
    }

    let ratio = WHISPER_SAMPLE_RATE / from_rate;
    let new_len = ((input.frame_count() as f64 * ratio).round() as usize).max(1);
    let mut output = vec![0.0f32; new_len];

    for i in 0..new_len {
        let src_pos = i as f64 / ratio;
        let src_idx = src_pos as usize;
        let frac = (src_pos - src_idx as f64) as f32;

        let mut sample0 = 0.0f32;
        for ch in 0..input.channel_count() {
            sample0 += input.channel(ch).get(src_idx).copied().unwrap_or(0.0);
        }
        sample0 /= input.channel_count().max(1) as f32;

        let mut sample1 = 0.0f32;
        for ch in 0..input.channel_count() {
            sample1 += input.channel(ch).get(src_idx + 1).copied().unwrap_or(sample0);
        }
        sample1 /= input.channel_count().max(1) as f32;

        output[i] = sample0 + (sample1 - sample0) * frac;
    }
    output
}

fn find_model_path() -> Option<String> {
    if let Ok(path) = std::env::var("MAKEPAD_VOICE_MODEL") {
        if std::path::Path::new(&path).exists() {
            return Some(path);
        }
    }

    let candidates = [
        "ggml-large-v3-turbo.bin",
        "../../ggml-large-v3-turbo.bin",
        "../../../ggml-large-v3-turbo.bin",
    ];

    for candidate in candidates {
        if std::path::Path::new(candidate).exists() {
            return Some(candidate.to_string());
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            for ancestor in exe_dir.ancestors().take(5) {
                let model_path = ancestor.join("ggml-large-v3-turbo.bin");
                if model_path.exists() {
                    return model_path.to_str().map(|s| s.to_string());
                }
            }
        }
    }

    None
}

fn transcribe_samples(samples: Vec<f32>, state: Arc<RecordingState>) {
    std::thread::spawn(move || {
        println!("Starting transcription of {} samples...", samples.len());

        let model_path = match find_model_path() {
            Some(path) => path,
            None => {
                let error_msg = "Model not found. Set MAKEPAD_VOICE_MODEL";
                println!("{}", error_msg);
                *state.transcription_result.lock().unwrap() = Some(format!("[Error: {}]", error_msg));
                return;
            }
        };

        println!("Using model: {}", model_path);

        match makepad_voice::WhisperModel::load_file(&model_path) {
            Ok(model) => {
                let mut whisper_state = makepad_voice::WhisperState::new(&model);
                let params = makepad_voice::WhisperParams::default();

                let segments = whisper_state.transcribe(&model, &samples, &params);

                let text: String = segments.iter()
                    .map(|s| s.text.trim())
                    .collect::<Vec<_>>()
                    .join(" ");

                println!("Transcription result: {}", text);
                *state.transcription_result.lock().unwrap() = Some(text);
            }
            Err(e) => {
                let error_msg = format!("Failed to load model: {:?}", e);
                println!("{}", error_msg);
                *state.transcription_result.lock().unwrap() = Some(format!("[Error]"));
            }
        }
    });
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,

    #[rust]
    state: Option<Arc<RecordingState>>,
    #[rust]
    audio_initialized: bool,
    #[rust]
    wave_update_timer: Timer,
    #[rust]
    current_amplitude: f32,
    #[rust]
    recording_start_time: Option<std::time::Instant>,
    #[rust]
    is_transcribing: bool,
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        self.state = Some(RecordingState::new());

        println!("Speech-to-Text with Waveform Visualizer");
        println!("Click the microphone button to start/stop recording");
        println!("\n=== Stdin Commands ===");
        println!("  r          - Toggle recording on/off");
        println!("  t          - Transcribe accumulated samples");
        println!("  w          - Test waveform with fake audio (5 sec)");
        println!("  s <amp>    - Set amplitude 0.0-1.0 (e.g., 's 0.5')");
        println!("  c          - Clear all samples");
        println!("  h          - Show help");
        println!("  q          - Quit");
        println!("======================\n");

        cx.use_audio_inputs(&[]);
        self.wave_update_timer = cx.start_interval(0.033); // ~30fps

        // Initialize waveform
        self.current_amplitude = 0.0;

        // Stdin commands
        let state = self.state.clone().unwrap();
        std::thread::spawn(move || {
            let stdin = std::io::stdin();
            let mut line = String::new();
            loop {
                line.clear();
                if stdin.read_line(&mut line).is_ok() {
                    let cmd = line.trim();
                    let parts: Vec<&str> = cmd.split_whitespace().collect();

                    match parts.get(0).copied().unwrap_or("") {
                        "r" => {
                            let was = state.is_recording.load(Ordering::SeqCst);
                            state.is_recording.store(!was, Ordering::SeqCst);
                            if was {
                                println!("[STDIN] Stopped recording");
                            } else {
                                state.accumulated_samples.lock().unwrap().clear();
                                state.recent_samples.lock().unwrap().clear();
                                println!("[STDIN] Started recording");
                            }
                        }
                        "t" => {
                            let samples = state.accumulated_samples.lock().unwrap().clone();
                            if !samples.is_empty() {
                                println!("[STDIN] Transcribing {} samples...", samples.len());
                                transcribe_samples(samples, state.clone());
                            } else {
                                println!("[STDIN] No samples to transcribe");
                            }
                        }
                        "w" => {
                            println!("[STDIN] Testing waveform with fake audio (5 seconds)...");
                            state.is_recording.store(true, Ordering::SeqCst);

                            // Generate 5 seconds of fake audio samples at 16kHz
                            let duration_secs = 5.0;
                            let sample_rate = WHISPER_SAMPLE_RATE as f32;
                            let num_samples = (duration_secs * sample_rate) as usize;
                            let mut fake_samples = Vec::with_capacity(num_samples);

                            for i in 0..num_samples {
                                let t = i as f32 / sample_rate;
                                // Generate a mix of frequencies for interesting waveform
                                let freq1 = 440.0; // A4 note
                                let freq2 = 880.0; // A5 note
                                let sample = (0.3 * (2.0 * std::f32::consts::PI * freq1 * t).sin()
                                           + 0.2 * (2.0 * std::f32::consts::PI * freq2 * t).sin())
                                           * (1.0 - t / duration_secs); // Fade out
                                fake_samples.push(sample);
                            }

                            // Add to accumulated samples
                            state.accumulated_samples.lock().unwrap().extend_from_slice(&fake_samples);

                            // Update recent samples for visualization (last 100ms)
                            let recent_count = (sample_rate * 0.1) as usize;
                            let start_idx = num_samples.saturating_sub(recent_count);
                            state.recent_samples.lock().unwrap().clear();
                            state.recent_samples.lock().unwrap().extend_from_slice(&fake_samples[start_idx..]);

                            println!("[STDIN] Generated {} fake samples. Type 'r' to stop, 't' to transcribe.", num_samples);
                        }
                        "s" => {
                            if let Some(amp_str) = parts.get(1) {
                                if let Ok(amplitude) = amp_str.parse::<f32>() {
                                    let clamped = amplitude.clamp(0.0, 1.0);
                                    println!("[STDIN] Setting amplitude to {:.2}", clamped);

                                    // Generate samples based on amplitude
                                    let sample_rate = WHISPER_SAMPLE_RATE as f32;
                                    let num_samples = (sample_rate * 0.1) as usize; // 100ms
                                    let mut samples: Vec<f32> = (0..num_samples)
                                        .map(|i| {
                                            let t = i as f32 / sample_rate;
                                            clamped * (2.0 * std::f32::consts::PI * 440.0 * t).sin()
                                        })
                                        .collect();

                                    state.recent_samples.lock().unwrap().clear();
                                    state.recent_samples.lock().unwrap().append(&mut samples);
                                } else {
                                    println!("[STDIN] Invalid amplitude. Use: s <0.0-1.0>");
                                }
                            } else {
                                println!("[STDIN] Usage: s <amplitude>  (e.g., s 0.5)");
                            }
                        }
                        "c" | "clear" => {
                            state.accumulated_samples.lock().unwrap().clear();
                            state.recent_samples.lock().unwrap().clear();
                            state.is_recording.store(false, Ordering::SeqCst);
                            println!("[STDIN] Cleared all samples and stopped recording");
                        }
                        "h" | "help" => {
                            println!("\n=== Stdin Commands ===");
                            println!("  r          - Toggle recording on/off");
                            println!("  t          - Transcribe accumulated samples");
                            println!("  w          - Test waveform with fake audio (5 sec)");
                            println!("  s <amp>    - Set amplitude 0.0-1.0 (e.g., s 0.5)");
                            println!("  c          - Clear all samples");
                            println!("  h          - Show help");
                            println!("  q          - Quit");
                            println!("======================\n");
                        }
                        "q" | "quit" | "exit" => {
                            println!("[STDIN] Exiting...");
                            std::process::exit(0);
                        }
                        "" => {} // Empty line, do nothing
                        _ => {
                            println!("[STDIN] Unknown command: '{}'. Type 'h' for help.", cmd);
                        }
                    }
                }
            }
        });
    }

    fn handle_audio_devices(&mut self, cx: &mut Cx, devices: &AudioDevicesEvent) {
        println!("Audio devices:");
        for desc in &devices.descs {
            println!("  {}", desc);
        }
        cx.use_audio_inputs(&devices.default_input());
        self.audio_initialized = true;
    }

    fn handle_timer(&mut self, cx: &mut Cx, e: &TimerEvent) {
        if self.wave_update_timer.is_timer(e).is_some() {
            if let Some(ref state) = self.state {
                let is_recording = state.is_recording.load(Ordering::SeqCst);

                // Calculate amplitude from recent samples
                let amplitude = {
                    let recent = state.recent_samples.lock().unwrap();
                    if recent.is_empty() {
                        0.0
                    } else {
                        let rms: f32 = (recent.iter().map(|s| s * s).sum::<f32>() / recent.len() as f32).sqrt();
                        (rms * 15.0).min(1.0)
                    }
                };

                // Smooth amplitude
                self.current_amplitude = self.current_amplitude * 0.7 + amplitude * 0.3;

                // Update waveform bars - show/hide based on amplitude
                let bar_count = (self.current_amplitude * 30.0) as usize;

                for i in 0..30 {
                    let bar_id = LiveId::from_str(&format!("bar_{}", i));
                    let is_active = i < bar_count;
                    self.ui.view(cx, &[bar_id]).set_visible(cx, is_active);
                }

                // Update mic button with amplitude and recording state
                if let Some(mut mic_button) = self.ui.widget(cx, ids!(mic_button)).borrow_mut::<MicButton>() {
                    mic_button.set_recording(cx, is_recording, self.current_amplitude);
                }

                // Show/hide loading spinner
                self.ui.view(cx, ids!(loading_spinner)).set_visible(cx, self.is_transcribing);

                // Update duration label
                if is_recording {
                    if let Some(start_time) = self.recording_start_time {
                        let elapsed = start_time.elapsed().as_secs_f32();
                        let minutes = (elapsed / 60.0) as u32;
                        let seconds = (elapsed % 60.0) as u32;
                        self.ui.label(cx, ids!(duration_label)).set_text(cx, &format!("{}:{:02}", minutes, seconds));
                    }
                } else {
                    self.ui.label(cx, ids!(duration_label)).set_text(cx, "");
                }

                // Check for transcription results
                if let Some(result) = state.transcription_result.lock().unwrap().take() {
                    println!("Transcription: {}", result);

                    self.is_transcribing = false;
                    println!("[DEBUG] Set is_transcribing = false");

                    // Append to existing text
                    let current_text = self.ui.text_input(cx, ids!(result_input)).text();
                    let new_text = if current_text.is_empty() {
                        result
                    } else {
                        format!("{} {}", current_text, result)
                    };
                    self.ui.text_input(cx, ids!(result_input)).set_text(cx, &new_text);
                    self.ui.label(cx, ids!(status_label)).set_text(cx, "Transcription complete - Click mic to continue");
                }
            }
        }
    }

    fn handle_signal(&mut self, cx: &mut Cx) {
        if let Some(ref state) = self.state {
            if let Some(result) = state.transcription_result.lock().unwrap().take() {
                println!("Transcription: {}", result);

                // Append to existing text
                let current_text = self.ui.text_input(cx, ids!(result_input)).text();
                let new_text = if current_text.is_empty() {
                    result
                } else {
                    format!("{} {}", current_text, result)
                };
                self.ui.text_input(cx, ids!(result_input)).set_text(cx, &new_text);

                script_eval!(cx, {
                    mod.state.status = "Transcription complete - Click mic to continue"
                    ui.main_view.render()
                });
            }
        }
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let state = match &self.state {
            Some(s) => s.clone(),
            None => return,
        };

        // Mic button clicked
        let mic_clicked = if let Some(mic_button) = self.ui.widget(cx, ids!(mic_button)).borrow::<MicButton>() {
            mic_button.clicked(actions)
        } else {
            false
        };

        if mic_clicked {
            let was_recording = state.is_recording.load(Ordering::SeqCst);
            state.is_recording.store(!was_recording, Ordering::SeqCst);

            if was_recording {
                // Stop and transcribe
                self.recording_start_time = None;
                let samples = state.accumulated_samples.lock().unwrap().clone();
                println!("Stopped. {} samples ({:.1}s)", samples.len(), samples.len() as f64 / WHISPER_SAMPLE_RATE);

                if !samples.is_empty() {
                    self.is_transcribing = true;
                    println!("[DEBUG] Set is_transcribing = true");
                    self.ui.label(cx, ids!(status_label)).set_text(cx, "Transcribing...");
                    transcribe_samples(samples, state.clone());
                }
            } else {
                // Start recording
                self.recording_start_time = Some(std::time::Instant::now());
                state.accumulated_samples.lock().unwrap().clear();
                state.recent_samples.lock().unwrap().clear();
                println!("Recording...");
                self.ui.label(cx, ids!(status_label)).set_text(cx, "Recording... Click mic to stop");
            }
        }

        // Clear button
        if self.ui.button(cx, ids!(clear_button)).clicked(actions) {
            self.recording_start_time = None;
            self.is_transcribing = false;
            state.accumulated_samples.lock().unwrap().clear();
            state.recent_samples.lock().unwrap().clear();
            *state.transcription_result.lock().unwrap() = None;
            self.ui.text_input(cx, ids!(result_input)).set_text(cx, "");
            self.current_amplitude = 0.0;
            self.ui.label(cx, ids!(status_label)).set_text(cx, "Cleared - Click mic to start");
        }
    }
}

impl App {
    pub fn start_audio_capture(&mut self, cx: &mut Cx) {
        let state = match &self.state {
            Some(s) => s.clone(),
            None => return,
        };

        cx.audio_input(0, move |info, input_buffer| {
            *state.sample_rate.lock().unwrap() = info.sample_rate;

            let resampled = resample_to_16k_mono(input_buffer, info.sample_rate);

            // Always update recent samples for waveform visualization
            let mut recent = state.recent_samples.lock().unwrap();
            recent.extend(resampled.iter());
            let max_recent = 1600; // 100ms at 16kHz
            let len = recent.len();
            if len > max_recent {
                recent.drain(0..len - max_recent);
            }
            drop(recent);

            // Only accumulate samples when recording for transcription
            if state.is_recording.load(Ordering::SeqCst) {
                state.accumulated_samples.lock().unwrap().extend(resampled.iter());
            }
        });
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
