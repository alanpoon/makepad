
use makepad_widgets::*;
live_design!{
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    pub LoadingSpinner2 = {{LoadingSpinner2}} {
        width: 40,
        height: 40,
        
        show_bg: true,
        draw_bg: {
            color: #f0e6e6,
            instance progress: 0.1,
            instance spinner_background_color: #6f6f6f,
            instance stroke_width: 4.0,
            instance gap_degrees: 60.0,
            
            instance border_size: 1.0,
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                let center = self.rect_size * 0.5;
                let radius = min(center.x, center.y) - self.stroke_width * 0.5 - self.border_size;

                // Draw background circle
                sdf.circle(center.x, center.y, radius);
                sdf.stroke(self.spinner_background_color, self.stroke_width);

                // Draw spinner arc
                let start_angle = self.progress * 2.0 * PI;
                let gap_radians = self.gap_degrees * PI / 180.0;
                sdf.arc_round_caps(
                    center.x, 
                    center.y, 
                    radius, 
                    start_angle, 
                    start_angle + 2.0 * PI - gap_radians, 
                    self.stroke_width
                );

                sdf.fill(self.color);
                return sdf.result;
            }
        }
        
        animator: {
            spin = {
                default: off,
                off = {
                    from: {all: Forward {duration: 0.0}}
                    apply: {
                        draw_bg: {progress: 0.0}
                    }
                }
                play = {
                    from: {all: Loop {duration: 1.0, end: 1.0}}
                    apply: {
                        draw_bg: {progress: [{time: 0.0, value: 0.0}, {time: 1.0, value: 1.0}]}
                    }
                }
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct LoadingSpinner2 {
    //#[deref] draw_bg: DrawLoadingSpinner2,
    #[animator] animator: Animator,
    #[walk] walk: Walk,
    #[layout] layout: Layout,
    #[deref] view: View,
    // #[redraw]
    // #[live]
    // draw_bg: DrawQuad,
}

impl Widget for LoadingSpinner2 {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.animator_handle_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }
    
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // self.draw_bg.begin(cx, walk, self.layout);
        // self.draw_bg.end(cx);
        self.view.draw_walk(cx, scope, walk);
        DrawStep::done()
    }
}

impl LoadingSpinner2Ref {
    pub fn start(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.animator_play(cx, id!(spin.play));
        }
    }
    
    pub fn stop(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            //inner.animator_stop(cx, id!(spin.play));
        }
    }
}