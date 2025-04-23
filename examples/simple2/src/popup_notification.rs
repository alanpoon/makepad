use std::any::Any;
use std::collections::HashMap;

use makepad_widgets::*;
live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    pub RobrixPopupNotification = {{RobrixPopupNotification}} {
        // Animators node: LiveNode { origin: token_id:Some(TokenId(token_index:30, file_id:80)) first_def:Some(TokenId(token_index:30, file_id:80)) edit_info:None prop_type:Field, id: animator, value: Object }
        // m LiveNode { origin: token_id:Some(TokenId(token_index:30, file_id:80)) first_def:Some(TokenId(token_index:30, file_id:80)) edit_info:None prop_type:Field, id: animator, value: Object }
        // v LiveNode { origin: token_id:Some(TokenId(token_index:33, file_id:80)) first_def:Some(TokenId(token_index:33, file_id:80)) edit_info:None prop_type:Instance, id: mode, value: Object }
        // v11 LiveNode { origin: token_id:Some(TokenId(token_index:36, file_id:80)) first_def:Some(TokenId(token_index:36, file_id:80)) edit_info:None prop_type:Field, id: default, value: Id(open) }
        // v2 LiveNode { origin: token_id:Some(TokenId(token_index:40, file_id:80)) first_def:Some(TokenId(token_index:40, file_id:80)) edit_info:None prop_type:Instance, id: open, value: Object }
        //                 LiveNode { origin: token_id:None first_def:None edit_info:None prop_type:Field, id: redraw, value: Bool(true) }
        // v3 LiveNode { origin: token_id:Some(TokenId(token_index:43, file_id:80)) first_def:Some(TokenId(token_index:43, file_id:80)) edit_info:None prop_type:Field, id: from, value: Object }
        // v44 LiveNode { origin: token_id:Some(TokenId(token_index:46, file_id:80)) first_def:Some(TokenId(token_index:46, file_id:80)) edit_info:None prop_type:Field, id: all, value: NamedEnum(Forward) }
        // v5 LiveNode { origin: token_id:Some(TokenId(token_index:50, file_id:80)) first_def:Some(TokenId(token_index:50, file_id:80)) edit_info:None prop_type:Field, id: duration, value: Float64(0.0) }

        // -----
        // [LiveNode { origin: token_id:None first_def:None edit_info:None prop_type:Field, id: 0, value: Object }, 
        // LiveNode { origin: token_id:None first_def:None edit_info:None prop_type:Field, id: animator, value: Object }, 
        // LiveNode { origin: token_id:None first_def:None edit_info:None prop_type:Instance, id: mode, value: Object }, 
        // LiveNode { origin: token_id:None first_def:None edit_info:None prop_type:Instance, id: mode, value: Close }, 
        // LiveNode { origin: token_id:None first_def:None edit_info:None prop_type:Field, id: animator, value: Close }, 
        // LiveNode { origin: token_id:None first_def:None edit_info:None prop_type:Field, id: 0, value: Close }]
        animator: {
            mode = {
                default: open,
                open = {
                    redraw: true,
                    from: {all: Forward {duration: 0.0}}
                }
                
            }
        }
    }
}

#[derive(Live, Widget)]
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

impl LiveHook for RobrixPopupNotification {
    fn before_apply(
        &mut self,
        _cx: &mut Cx,
        apply: &mut Apply,
        index: usize,
        nodes: &[LiveNode],
    ) {
        if let ApplyFrom::UpdateFromDoc { .. } = apply.from {
            // First, find the 'animator' field
            if let Some(animator_index) = nodes.child_by_name(index, live_id!(animator).as_field()) {
                println!("Animators node: {:?}", nodes[animator_index]);
                println!("m {:?}", nodes[animator_index]);
                println!("v {:?}", nodes[animator_index+1]);
                println!("v11 {:?}", nodes[animator_index+2]);
                println!("v2 {:?}", nodes[animator_index+3]);
                println!("v3 {:?}", nodes[animator_index+4]);
                println!("v44 {:?}", nodes[animator_index+5]);
                println!("v555 {:?}", nodes[animator_index+6]);
            }
                
                
        }
    }
    fn after_apply(&mut self, cx: &mut Cx, _apply: &mut Apply, _index: usize, _nodes: &[LiveNode]) {
        self.draw_list.redraw(cx);
    }
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
        // self.animation_timer = cx.start_timeout(4.0);
        // self.set_duration(cx, 0.0);
        // self.view(id!(progress)).animator_play(cx, id!(mode.progress));
        //self.animator_play(cx, id!(mode.open));s
        let b = live!{
            animator: {
                mode = {
                    progress = {
                        redraw: true,
                        from: {all: Play::Forward {duration: 0.0}},
                        apply: {
                            progress_bar = {
                                height: 100,
                            }
                        }
                    }
                }
            }
        };
        let b = &[
                LiveNode {
                    origin: LiveNodeOrigin::empty(),
                    id: LiveId(0),
                    value: LiveValue::Object,
                },
                LiveNode {
                    origin: LiveNodeOrigin::field(),
                    id: LiveId(17147931769487475859u64), //animator
                    value: LiveValue::Object,
                },
                LiveNode {
                    origin: LiveNodeOrigin::instance(),
                    id: LiveId(14545868489622790003u64), // mode
                    value: LiveValue::Object,
                },
                LiveNode {
                    origin: LiveNodeOrigin::instance(),
                    id: LiveId(15793447310906918479u64), //progress
                    value: LiveValue::Object,
                },
                LiveNode {
                    origin: LiveNodeOrigin::field(),
                    id: LiveId(15337755572090582118u64), //redraw
                    value: LiveValue::Bool(true),
                },
                LiveNode {
                    origin: LiveNodeOrigin::field(),
                    id: LiveId(16904472783508785295u64), //from
                    value: LiveValue::Object,
                },
                LiveNode {
                    origin: LiveNodeOrigin::field(),
                    id: LiveId(16774991215892966969u64), //all
                    value: LiveValue::NamedEnum(LiveId(17468313455979708392u64)), //forward
                },
                LiveNode {
                    origin: LiveNodeOrigin::field(),
                    id: LiveId(11712834513200702032u64), //duration
                    value: LiveValue::Float64(0.0),
                },
                LiveNode {
                    origin: LiveNodeOrigin::field(),
                    id: LiveId(16774991215892966969u64), //all
                    value: LiveValue::Close,
                },
                LiveNode {
                    origin: LiveNodeOrigin::field(),
                    id: LiveId(16904472783508785295u64), //from
                    value: LiveValue::Close,
                },
                LiveNode {
                    origin: LiveNodeOrigin::field(),
                    id: LiveId(16367865674581407327u64), //apply
                    value: LiveValue::Object,
                },
                LiveNode {
                    origin: LiveNodeOrigin::field(),
                    id: LiveId(14826783821801388196u64), //cdc351548bfaf8a4
                    value: LiveValue::Object,
                },
                LiveNode {
                    origin: LiveNodeOrigin::field(),
                    id: LiveId(15099229944098024443u64), //height
                    value: LiveValue::Int64(100),
                },
                LiveNode {
                    origin: LiveNodeOrigin::field(),
                    id: LiveId(14826783821801388196u64), //cdc351548bfaf8a4
                    value: LiveValue::Close,
                },
                LiveNode {
                    origin: LiveNodeOrigin::field(),
                    id: LiveId(16367865674581407327u64), //apply
                    value: LiveValue::Close,
                },
                LiveNode {
                    origin: LiveNodeOrigin::instance(),
                    id: LiveId(15793447310906918479u64), //progress
                    value: LiveValue::Close,
                },
                LiveNode {
                    origin: LiveNodeOrigin::instance(),
                    id: LiveId(14545868489622790003u64), //mode
                    value: LiveValue::Close,
                },
                LiveNode {
                    origin: LiveNodeOrigin::field(),
                    id: LiveId(17147931769487475859u64), //animator
                    value: LiveValue::Close,
                },
                LiveNode {
                    origin: LiveNodeOrigin::empty(),
                    id: LiveId(0),
                    value: LiveValue::Close,
                },
            ];
            let mut c= 0;
          for i in b {
            println!("c{:?} id {:?} ",c, i.id.to_string() );
            match i.value{
                LiveValue::NamedEnum(b) => { println!("value {:?}", b.to_string())}
                _ => {}
            }
            c+=1;
          }
        println!("B {:?}",b);

        //self.redraw(cx);
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
        self.animator.swap_in_state(live!{
            mode = {
                progress = {
                    redraw: true,
                    from: {all: Play::Forward {duration: 0.0}},
                    apply: {
                        progress_bar = {
                            height: 100,
                        }
                    }
                }
            }
        }.to_vec());
        
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