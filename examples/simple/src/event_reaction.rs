use makepad_widgets::*;


live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;
    COLOR_BUTTON_DARKER = #454343
    ReactionList = {{ReactionList}} {

        // item: <Button> {
        //     width: Fit,
        //     height: Fit,
        //     spacing: 20,
        //     padding: {top: 3, bottom: 3, left: 3, right: 3}
        //     margin: {
        //         top:3,
        //         bottom:3,
        //         left:3,
        //         right:3

        //     },
        //     draw_bg: {
        //         instance color: (COLOR_BUTTON_DARKER)
        //         instance color_hover: #fef65b
        //         instance border_width: 0.0
        //         instance border_color: #D0D5DD
        //         instance radius: 3.0

        //         fn get_color(self) -> vec4 {
        //             return mix(self.color, mix(self.color, self.color_hover, 0.2), self.hover)
        //         }

        //         fn pixel(self) -> vec4 {
        //             let sdf = Sdf2d::viewport(self.pos * self.rect_size)
        //             sdf.box(
        //                 self.border_width,
        //                 self.border_width,
        //                 self.rect_size.x - (self.border_width * 2.0),
        //                 self.rect_size.y - (self.border_width * 2.0),
        //                 max(1.0, self.radius)
        //             )
        //             sdf.fill_keep(self.get_color())
        //             if self.border_width > 0.0 {
        //                 sdf.stroke(self.border_color, self.border_width)
        //             }
        //             return sdf.result;
        //         }
        //     }

        //     draw_icon: {
        //         instance color: #000
        //         instance color_hover: #000
        //         uniform rotation_angle: 0.0,

        //         fn get_color(self) -> vec4 {
        //             return mix(self.color, mix(self.color, self.color_hover, 0.2), self.hover)
        //         }


        //     }
        //     icon_walk: {width: 16, height: 16}

        //     draw_text: {
        //         text_style: <REGULAR_TEXT>{font_size: 8},
        //         color: #ffffff
        //         fn get_color(self) -> vec4 {
        //             return self.color;
        //         }
        //     }
        // }
    }
}

/// Actions that can be performed on a message.
#[derive(Clone, DefaultNone, Debug)]
pub enum ReactionListAction {
    //Clicked index
    Clicked(usize),
    None
}
#[derive(Live, Widget)]
pub struct ReactionList{
    #[redraw]
    #[rust]
    area: Area,
    #[live]
    item: Option<LivePtr>,
    #[rust]
    children: ComponentMap<LiveId, ButtonRef>,
    // #[rust]
    // children: ComponentMap<LiveId, ViewRef>,
    #[layout]
    layout: Layout,
    #[walk]
    walk: Walk,
    #[rust]
    pub list: Vec<(String, usize)>,
    // #[rust]
    // pub room_id: Option<OwnedRoomId>,
    #[rust]
    pub unique_id: Option<String>,
    #[rust]
    pub draw_walk_callback: Option<Box<dyn Fn(&mut ButtonRef,usize)>>,
    #[rust]
    pub handle_actions_callback: Option<Box<dyn Fn(&ViewRef,&Vec<Box<dyn ActionTrait>>,usize)>>,
    #[rust]
    pub grid_system: Vec<Vec<usize>>
}
impl Widget for ReactionList {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        println!("draw_walk ");
        cx.begin_turtle(walk, self.layout);
        let rect = cx.turtle().rect();
        let width: f64 = rect.size.x - 50.0;
        let mut acc_width: f64 = 0.0;
        let mut acc_height = 0.0;
        for (index, (emoji, count)) in self.list.iter().enumerate() {
            println!("index {:?}",index);
            let target = self.children.get_or_insert(cx, LiveId(index as u64), |cx| {
                WidgetRef::new_from_ptr(cx, self.item).as_button()
                //WidgetRef::new_from_ptr(cx, self.item).as_view()
            });
            if let Some(closure) = &self.draw_walk_callback {
                (closure)(target, index);
            }
            //target.set_text(&format!("{} {}", emoji, count));
            //target.label(id!(text_label)).set_text(&format!("{} {}", emoji, count));
            target.draw_all(cx, scope);
            let used = cx.turtle().used();
            acc_width = used.x;
            println!("acc_width  {:?}> width {:?}",acc_width, width);
            if acc_width > width {
                cx.turtle_new_line();
                target.redraw(cx);
                let used = cx.turtle().used();
                acc_height = used.y;
                cx.turtle_mut().set_used(0.0, used.y);
            }
            if acc_height == 0.0 {
                acc_height = used.y;
            }
        }
        cx.end_turtle();
        self.children.retain_visible();
        DrawStep::done()
    }
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        //let Some(room_id) = &self.room_id else { return };
        println!("handle_event ");
        let Some(unique_id) = &self.unique_id else {
            return;
        };
        let widget_uid = self.widget_uid();
        self.children
            .iter()
            .enumerate()
            .for_each(|(index, (_id, widget_ref))| {
                widget_ref.handle_event(cx, event, scope);
                match event {
                    Event::Actions(actions) => {
                        // if let Some(closure) = &self.handle_actions_callback {
                        //     (closure)(widget_ref,actions,index);
                        // }
                        // if let Some(_) = widget_ref.finger_down(actions) {
                        //     cx.widget_action(
                        //         widget_uid,
                        //         &scope.path,
                        //         ReactionListAction::Clicked(index)
                        //     );
                        // }
                        // if widget_ref.clicked(&actions) {
                        //     cx.widget_action(
                        //         widget_uid,
                        //         &scope.path,
                        //         ReactionListAction::Clicked(index)
                        //     );
                        //     // let text = widget_ref.text().clone();
                        //     // let mut reaction_string_arr: Vec<&str> = text.split(" ").collect();
                        //     // reaction_string_arr.pop();
                        //     // let reaction_string = reaction_string_arr.join(" ");
                        //     // if let Some(key) = emojis::get_by_shortcode(&reaction_string) {
                        //     //     submit_async_request(MatrixRequest::ToggleReaction {
                        //     //         room_id: room_id.clone(),
                        //     //         unique_id: unique_id.clone(),
                        //     //         reaction_key: key.as_str().to_string(),
                        //     //     });
                        //     // }
                        // }
                    }
                    _ => {}
                }
            });
    }
}
impl LiveHook for ReactionList {
    fn before_apply(&mut self, cx: &mut Cx, apply: &mut Apply, index: usize, nodes: &[LiveNode]) {
        self.list = vec![(String::from("qq"),2),(String::from("ww"),3),(String::from("ll"),2),(String::from("ww"),3),(String::from("cc"),2),(String::from("ww"),3),(String::from("yy"),2),(String::from("ww"),3),(String::from("uu"),2),(String::from("nn"),3)];
        let l = self.list.clone();
        self.draw_walk_callback = Some(Box::new(move |vr,index|{
            let s = l.get(index).unwrap();
            vr.set_text(&format!("{} {}", s.0 , index));
        }));
        self.handle_actions_callback = Some(Box::new(move |vr,actions,index|{
            if let Some(_) = vr.finger_down(actions) {
                println!("presed {:?}", index);
            }
        }));
    }
}

impl ReactionListRef {
    pub fn set_list(
        &mut self,
        looper: Vec<(String,usize)>,
        // looper: &ReactionsByKeyBySender,
        // room_id: OwnedRoomId,
        unique_id: &str,
    ) {
        if let Some(mut instance) = self.borrow_mut() {
            instance.list = looper;
            //let mut text_to_display_vec = Vec::with_capacity(looper.len());
            // for (reaction_raw, reaction_senders) in looper.iter() {
            //     // Just take the first char of the emoji, which ignores any variant selectors.
            //     let reaction_first_char = reaction_raw.chars().next().map(|c| c.to_string());
            //     let reaction_str = reaction_first_char.as_deref().unwrap_or(reaction_raw);
            //     let text_to_display = emojis::get(reaction_str)
            //         .and_then(|e| e.shortcode())
            //         .unwrap_or(reaction_raw);
            //     let count = reaction_senders.len();
            //     text_to_display_vec.push((text_to_display.to_string(), count));
            // }
            // instance.list = text_to_display_vec;
            // instance.room_id = Some(room_id);
            // instance.unique_id = Some(unique_id.to_string());
        }
    }
}
