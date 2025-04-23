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
                    from: {all: Forward {duration: 4.0}}
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
                    from: {all: Forward {duration: (20.0) }}
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
    #[live] animator: Animator,
    #[rust] open: bool,
}


impl Widget for RobrixPopupNotification {
    
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let b = live!{

        };
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
impl AnimatorImpl for RobrixPopupNotification {
    fn animator_play_with_scope(
        &mut self,
        cx: &mut Cx,
        state: &[LiveId; 2],
        scope: &mut Scope,
    ) {
        self.animator.animate_to_live(cx, state);
        self.animator_apply_state(cx, scope);
    }
    fn animator_in_state(&self, cx: &Cx, check_state_pair: &[LiveId; 2]) -> bool {
        self.animator.animator_in_state(cx, check_state_pair)
    }
    fn animator_cut_with_scope(
        &mut self,
        cx: &mut Cx,
        state: &[LiveId; 2],
        scope: &mut Scope,
    ) {
        self.animator.cut_to_live(cx, state);
        self.animator_apply_state(cx, scope);
    }
    fn animator_after_apply(
        &mut self,
        cx: &mut Cx,
        apply: &mut Apply,
        index: usize,
        nodes: &[LiveNode],
    ) {
        let mut index = index + 1;
        match apply.from {
            ApplyFrom::NewFromDoc { .. } => {
                while !nodes[index].is_close() {
                    if let Some(LiveValue::Id(default_id)) = nodes
                        .child_value_by_path(
                            index,
                            &[LiveId(14728911141581693418u64).as_field()],
                        )
                    {
                        if let Some(index) = nodes
                            .child_by_path(
                                index,
                                &[
                                    default_id.as_instance(),
                                    LiveId(16367865674581407327u64).as_field(),
                                ],
                            )
                        {
                            apply
                                .override_from(
                                    ApplyFrom::AnimatorInit,
                                    |apply| {
                                        if !<Self as LiveHook>::skip_apply_animator(
                                            self,
                                            cx,
                                            apply,
                                            index,
                                            nodes,
                                        ) {
                                            let mut state = nodes.to_vec();
                                            for child in state.iter_mut() {
                                                println!("{:?}", child.id.to_string());
                                                if child.id.to_string() == "duration" {
                                                    println!("child.value {:?}", child.value);
                                                    child.value = LiveValue::Float64(8.0);
                                                }
                                            }
                                            self.apply(cx, apply, index, &state);
                                        }
                                    },
                                );
                        }
                    }
                    index = nodes.skip_node(index);
                }
            }
            ApplyFrom::UpdateFromDoc { .. } => {
                while !nodes[index].is_close() {
                    if let Some(LiveValue::Id(default_id)) = nodes
                        .child_value_by_path(
                            index,
                            &[LiveId(14728911141581693418u64).as_field()],
                        )
                    {
                        if let Some(index) = nodes
                            .child_by_path(
                                index,
                                &[
                                    default_id.as_instance(),
                                    LiveId(16367865674581407327u64).as_field(),
                                ],
                            )
                        {
                            apply
                                .override_from(
                                    ApplyFrom::AnimatorInit,
                                    |apply| {
                                        if !<Self as LiveHook>::skip_apply_animator(
                                            self,
                                            cx,
                                            apply,
                                            index,
                                            nodes,
                                        ) {
                                            let mut state = nodes.to_vec();
                                            for child in state.iter_mut() {
                                                println!("{:?}", child.id.to_string());
                                                if child.id.to_string() == "duration" {
                                                    child.value = LiveValue::Float64(8.0);
                                                }
                                            }
                                            self.apply(cx, apply, index, &state);
                                            //self.apply(cx, apply, index, nodes);
                                        }
                                    },
                                );
                        }
                    }
                    if let Some(scope) = &mut apply.scope {
                        self.animator_apply_state(cx, *scope);
                    } else {
                        self.animator_apply_state(cx, &mut Scope::empty());
                    }
                    index = nodes.skip_node(index);
                }
            }
            ApplyFrom::AnimatorInit => {
                if let Some(live_ptr) = self.animator.live_ptr {
                    let live_registry_rc = cx.live_registry.clone();
                    let live_registry = live_registry_rc.borrow();
                    if live_registry.generation_valid(live_ptr) {
                        let (orig_nodes, orig_index) = live_registry
                            .ptr_to_nodes_index(live_ptr);
                        while !nodes[index].is_close() {
                            if let LiveValue::Id(state_id) = nodes[index].value {
                                if let Some(orig_index) = orig_nodes
                                    .child_by_path(
                                        orig_index,
                                        &[
                                            nodes[index].id.as_instance(),
                                            state_id.as_instance(),
                                            LiveId(16367865674581407327u64).as_field(),
                                        ],
                                    )
                                {
                                    apply
                                        .override_from(
                                            ApplyFrom::AnimatorInit,
                                            |apply| {
                                                if !<Self as LiveHook>::skip_apply_animator(
                                                    self,
                                                    cx,
                                                    apply,
                                                    orig_index,
                                                    nodes,
                                                ) {
                                                    let mut state = nodes.to_vec();
                                            for child in state.iter_mut() {
                                                println!("{:?}", child.id.to_string());
                                                if child.id.to_string() == "duration" {
                                                    child.value = LiveValue::Float64(8.0);
                                                }
                                            }
                                                    //self.apply(cx, apply, orig_index, orig_nodes);
                                                    self.apply(cx, apply, orig_index, &state);
                                                }
                                            },
                                        );
                                }
                            }
                            index = nodes.skip_node(index);
                        }
                    }
                }
            }
            ApplyFrom::Animate => {
                while !nodes[index].is_close() {
                    let state_id = LiveId::new_apply(
                        cx,
                        &mut ApplyFrom::New.into(),
                        index,
                        nodes,
                    );
                    let state_pair = &[nodes[index].id, state_id];
                    if !self.animator.animator_in_state(cx, state_pair) {
                        self.animator.animate_to_live(cx, state_pair);
                    }
                    index = nodes.skip_node(index);
                }
            }
            _ => {}
        }
    }
    fn animator_apply_state(&mut self, cx: &mut Cx, scope: &mut Scope) {
        if let Some(state) = self.animator.swap_out_state() {
            let index = state
                .child_by_name(0, LiveId(17503461710418361181u64).as_field())
                .unwrap();
            println!("index: {} {} state {:?}", index, LiveId(17503461710418361181u64).to_string(), state);
            let mut apply = ApplyFrom::Animate.with_scope(scope);
            if !<Self as LiveHook>::skip_apply_animator(
                self,
                cx,
                &mut apply,
                index,
                &state,
            ) {
                
                self.apply(cx, &mut apply, index, &state);
            }
            let mut state = state.to_vec();
                for child in state.iter_mut() {
                    println!("{:?}", child.id.to_string());
                    if child.id.to_string() == "duration" {
                        println!("child.value {:?}", child.value);
                        child.value = LiveValue::Float64(8.0);
                    }
                }
            self.animator.swap_in_state(state);
        }
    }
    fn animator_handle_event_with_scope(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        scope: &mut Scope,
    ) -> AnimatorAction {
        let ret = self.animator.handle_event(cx, event);
        if ret.is_animating() {
            self.animator_apply_state(cx, scope);
        }
        ret
    }
}