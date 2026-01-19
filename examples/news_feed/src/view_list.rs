use makepad_widgets::*;
live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    COLOR_DIVIDER = #x00000010
    pub ViewList = {{ViewList}} {
        width: 275
        height: Fit
        flow: Down
        content: <RoundedView> {
            width: 10, height: 40
            show_bg: true
            flow: Down
            draw_bg: {color: (#32a852)}
        }
    }

}

#[derive(Live, LiveHook, Widget)]
pub struct ViewList {
     #[live]
    pub content: Option<LivePtr>,
    #[redraw]
    #[live]
    draw_bg: DrawQuad,
    #[layout]
    layout: Layout,
    #[walk]
    walk: Walk,
    #[rust]
    view_list: Vec<View>,
}
impl ViewList {
    pub fn set_view_list(&mut self, view_list: Vec<View>) {
        self.view_list = view_list;
    }
    pub fn get_content_template(&self) -> Option<LivePtr> {
        self.content
    }
}
impl ViewListRef {
    pub fn set_view_list(&mut self, view_list: Vec<View>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_view_list(view_list);
        } else {
            log!("ViewList is not initialized.");
        }
    }
    pub fn get_content_template(&self) -> Option<LivePtr> {
        if let Some(inner) = self.borrow() {
            inner.get_content_template()
        } else {
            None
        }
    }
}
// impl LiveHook for ViewList {
//     fn after_apply(&mut self, cx: &mut Cx, apply: &mut Apply, index: usize, nodes: &[LiveNode]) {
//         let mut holder = vec![];
//         for _ in 0..3 {
//             let view = View::new_from_ptr(cx, self.content);
//             holder.push(view);
//         }
//         self.view_list = holder;
//     }
// }
impl Widget for ViewList {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.draw_bg.begin(cx, walk, self.layout);
        cx.begin_turtle(walk, self.layout);
        for (_, view) in self.view_list.iter_mut().enumerate() {
            let _ = view.draw_walk(cx, scope, view.walk);
        }
        cx.end_turtle();
        self.draw_bg.end(cx);
        DrawStep::done()
    }
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        for view in self.view_list.iter_mut() {
            let _ = view.handle_event(cx, event, scope);
        }
    }
}