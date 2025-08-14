use makepad_widgets::*;

live_design!{
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    
    ZoomableImage = {{ZoomableImage}} {
        draw_bg: {
            instance image_scale: vec2(10.0, 1.0)
            instance image_pan: vec2(0.0, 0.0)
            
            // fn pixel(self) -> vec4 {
            //     let uv = self.pos / self.rect_size;
                
            //     // Apply scaling and panning
            //     let scaled_uv = (uv - 0.5) / self.image_scale + 0.5 + self.image_pan;
                
            //     // Check bounds - show dark background outside image
            //     if scaled_uv.x < 0.0 || scaled_uv.x > 1.0 || scaled_uv.y < 0.0 || scaled_uv.y > 1.0 {
            //         return #2e2e2e;
            //     }
                
            //     return sample2d(self.image, scaled_uv);
            // }
        }
    }
    
    App = {{App}} {
        ui: <Root>{
            main_window = <Window>{
                window: {position: vec2(0, 0), inner_size: vec2(800, 600)},
                pass: {clear_color: #1e1e1e}
                
                body = <View> {
                    width: Fill,
                    height: Fill,
                    flow: Down,
                    
                    // Instructions
                    <View> {
                        width: Fill,
                        height: Fit,
                        padding: 10.0,
                        align: {x: 0.5}
                        
                        <Label> {
                            text: "Image Viewer - Click and drag to pan, +/- to zoom, 0 to reset",
                            draw_text: {
                                text_style: { font_size: 12.0 },
                                color: #bbb,
                            }
                        }
                    }
                    
                    // Image container
                    image_container = <View> {
                        width: Fill,
                        height: Fill,
                        
                        zoomable_image = <Image> {
                            source: dep("crate://self/resources/hassaan-here-Ype8P9pAjXQ-unsplash.jpg"),
                            width: Fill,
                            height: Fill,
                            draw_bg: {
                                instance image_scale: vec2(1.0, 1.0)
                                instance image_pan: vec2(0.0, 0.0)
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct ZoomableImage {
    #[deref] image: Image,
}
impl Widget for ZoomableImage {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope:&mut Scope, mut walk: Walk) -> DrawStep {
        self.image.draw_walk(cx, walk)
    }
}
app_main!(App); 

#[derive(Live, LiveHook)]
pub struct App {
    #[live] ui: WidgetRef,
    #[rust] is_dragging: bool,
    #[rust] drag_start: DVec2,
    #[rust] zoom_level: f64,
    #[rust] pan_offset: DVec2,
}

impl Default for App {
    fn default() -> Self {
        Self {
            ui: WidgetRef::default(),
            is_dragging: false,
            drag_start: DVec2::default(),
            zoom_level: 1.0,
            pan_offset: DVec2::default(),
        }
    }
}

impl LiveRegister for App {
    fn live_register(cx: &mut Cx) { 
        
        crate::makepad_widgets::live_design(cx);
    }
}

impl MatchEvent for App{
    fn handle_actions(&mut self, _cx: &mut Cx, _actions: &Actions) {
        // Handle UI actions here
    }
}

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
        
        match event {
            Event::MouseDown(e) if e.button.is_primary() => {
                self.is_dragging = true;
                self.drag_start = e.abs;
                log!("Started dragging at {:?}", e.abs);
            }
            Event::MouseUp(e) if e.button.is_primary() => {
                self.is_dragging = false;
                log!("Stopped dragging");
            }
            Event::MouseMove(e) => {
                if self.is_dragging {
                    let delta = e.abs - self.drag_start;
                    if self.zoom_level == 0.0 {
                        self.zoom_level = 1.0;
                    }
                    self.pan_offset += delta * -0.001 * self.zoom_level; // Scale movement by zoom level
                    self.drag_start = e.abs;
                    log!("Panning: delta={:?}, total_offset={:?} self.zoom_level {:?}", delta, self.pan_offset, self.zoom_level);
                    self.update_image_shader(cx);
                }
            }
            Event::KeyDown(e) => {
                match &e.key_code {
                    KeyCode::Equals | KeyCode::NumpadAdd => {
                        self.zoom_level = (self.zoom_level * 1.2).min(5.0);
                        if self.zoom_level == 0.0 {
                            self.zoom_level = 1.0;
                        }
                        log!("Zoom in to {:.2}", self.zoom_level);
                        self.update_image_shader(cx);
                    }
                    KeyCode::Minus | KeyCode::NumpadSubtract => {
                        self.zoom_level = (self.zoom_level / 1.2).max(0.2);
                        if self.zoom_level == 0.0 {
                            self.zoom_level = 1.0;
                        }
                        log!("Zoom out to {:.2}", self.zoom_level);
                        self.update_image_shader(cx);
                    }
                    KeyCode::Key0 | KeyCode::Numpad0 => {
                        self.zoom_level = 1.0;
                        self.pan_offset = DVec2::default();
                        log!("Reset view");
                        self.update_image_shader(cx);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl App {
    fn update_image_shader(&mut self, cx: &mut Cx) {
        // Get the zoomable image widget and update its shader uniforms
        let mut zoomable_image = self.ui.zoomable_image(id!(zoomable_image));
        zoomable_image.apply_over(cx, live!{
            draw_bg: {
                image_scale: (self.zoom_level),
                image_pan: (self.pan_offset)
            }
        });
        
        // Request a redraw
        zoomable_image.redraw(cx);
    }
}