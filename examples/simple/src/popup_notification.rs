use std::any::Any;
use std::collections::HashMap;

use makepad_widgets::*;
use makepad_widgets::Play::Forward;
live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    ICO_CLOSE = dep("crate://self/resources/icons/close.svg")
    ICO_CHECK = dep("crate://self/resources/icons/checkmark.svg")

    Progress = <View> {
        width: 21,
        height: Fill,
        flow: Overlay,

        <RoundedView> {
            width: Fill,
            height: Fill,
            draw_bg: {
                color: #42660a,
            }
        }

        progress_bar = <RoundedView> {
            height: 0,
            width: Fill,
            draw_bg: {
                color: #639b0d,
            }
        }
        animator: {
            mode = {
                default: close,
                close = {
                    redraw: true,
                    from: {all: Forward {duration: 0.0}}
                    apply: {
                        progress_bar = {
                            height: -100,
                        }
                    }
                }
                progress = {
                    redraw: true,
                    //from: {all: Forward {duration: 4.0}}
                    from: {all: Snap}
                    apply: {
                        progress_bar = {
                            height: 100,
                        }
                    }
                }
            }
        }
    }

    TipContent = <View> {
        width: Fill,
        height: Fill,
        spacing: 15.0,
        flow: Right,
        align: {
            x: 0.0,
            y: 0.5,
        }
        margin: { left: 20.0 }

        <Icon> {
            draw_icon: {
                svg_file: (ICO_CHECK),
                color: #42660a,
            }
            icon_walk: { width: 18, height: 18 }
        }

        <Label> {
            draw_text: {
                color: #42660a,
                text_style: {
                    font_size: 12
                }
            }
            text: "Successfully updated transaction",
        }

        close_icon = <View> {
            width: Fit,
            height: Fit,
            cursor: Hand,
            <Icon> {
                draw_icon: {
                    svg_file: (ICO_CLOSE),
                    color: #6cc328
                }

                icon_walk: { width: 16, height: 16 }
            }
        }

    }

    PopupDialog = <RoundedView> {
        width: 375,
        height: 100,
        flow: Right,

        show_bg: true,
        draw_bg: {
            color: #d3f297,
        }
        l = <Label> {
            draw_text: {
                text_style: {font_size: 9}
            }
            text: "dfafokak"
        }
        progress = <Progress> {}
        <TipContent> {}
    }

    pub RobrixPopupNotification = {{RobrixPopupNotification}} {
        width: Fit
        height: Fit
        flow: Overlay
        abs_pos: vec2(10.0, 10.0)
        duration: 2.0

        draw_bg: {
            fn pixel(self) -> vec4 {
                return vec4(0., 0., 0., 0.0)
            }
        }

        content: <PopupDialog> {}
        
        animator: {
            mode = {
                default: close,
                open = {
                    redraw: true,
                    from: {all: Forward {duration: 2.0}}
                    ease: OutQuad
                    apply: {
                        abs_pos: vec2(60.0, 10.0),
                        // This didn't work
                        //content = { progress2 = { progress_bar = { height: -100 } } } 
                    }
                }
                progress = {
                    redraw: true,
                    from: {all: Forward {duration: (2.0) }}
                    ease: OutQuad
                    apply: {
                        abs_pos: vec2(60.0, 10.0),
                        // This didn't work
                        //content = { progress2 = { progress_bar = { height: 100 } } } 

                    }
                }
                close = {
                    redraw: true,
                    from: {all: Forward {duration: 1.0}}
                    ease: InQuad
                    apply: {
                        abs_pos: vec2(-1000.0, 10.0),
                    }
                }
            }
        }
    }
}

#[derive(Live, Widget, LiveHook)]
pub struct RobrixPopupNotification {
    #[live]
    #[find]
    content: View,

    #[live]
    duration: f64,

    #[rust(DrawList2d::new(cx))]
    draw_list: DrawList2d,

    #[redraw]
    #[live]
    draw_bg: DrawQuad,
    #[layout]
    layout: Layout,
    #[walk]
    walk: Walk,

    #[rust]
    animation_timer: Timer,
    #[animator]
    animator: Animator,
    #[rust] templates: ComponentMap<LiveId, LivePtr>,
    #[rust] items: ComponentMap<usize, WidgetItem>,
    #[rust] open: bool,
}
struct WidgetItem{
    widget: WidgetRef,
    template: LiveId,
}


impl Widget for RobrixPopupNotification {
    
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.animation_timer.is_event(event).is_some() {
            self.close(cx);
           
        }

        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }

        if let Event::MouseDown(e) = event {
            if self.view(id!(close_icon)).area().rect(cx).contains(e.abs) {
                self.close(cx);
                return;
            }
        }

        self.content.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, _walk: Walk) -> DrawStep {
        self.draw_list.begin_overlay_reuse(cx);

        cx.begin_pass_sized_turtle(self.layout);
        self.draw_bg.begin(cx, self.walk, self.layout);
        self.content.draw_all(cx, scope);
        self.draw_bg.end(cx);

        cx.end_pass_sized_turtle();
        self.draw_list.end(cx);

        DrawStep::done()
    }
}

impl RobrixPopupNotification {
    pub fn open(&mut self, cx: &mut Cx) {
        self.animation_timer = cx.start_timeout(4.0);
        self.set_duration(cx, 0.0);
        self.view(id!(progress)).animator_play(cx, id!(mode.progress));
        self.animator_play(cx, id!(mode.open));
        self.redraw(cx);
        self.open = true;
    }

    pub fn close(&mut self, cx: &mut Cx) {
        self.animator_play(cx, id!(mode.close));
        self.view(id!(progress)).animator_play(cx, id!(mode.close));
        self.redraw(cx);
    }
    pub fn set_duration(&mut self, cx:&mut Cx, duration: f64) {
        println!("set_duration");
        println!("{:?}",self.animator.live_ptr);
        println!("state {:?}", self.animator.type_id());
        for (live_file) in cx
            .live_registry
            .clone()
            .borrow_mut()
            .live_files.iter_mut() {
                for live_node in &mut live_file.expanded.nodes {
                    if live_node.id.to_string() == "duration" {
                        live_node.value = LiveValue::Float64(9.0);
                    }
                }
            }
        //println!("compone {:?}",cx.live_registry.borrow_mut().components.0.len());
        // let mut state = self.animator.state.clone().unwrap();
        // for child in state.iter_mut() {
        //     if child.id.to_string() == "duration" {
        //         child.value = LiveValue::Float64(8.0);
        //     }
        // }
        // self.animator.swap_in_state(state);
    }
}

impl RobrixPopupNotificationRef {
    pub fn open(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            
            inner.open(cx);
            
        }
    }

    pub fn close(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.close(cx);
        }
    }
}