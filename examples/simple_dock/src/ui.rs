use makepad_widgets::*;

live_design!(
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    pub Ui = {{Ui}} {
        align: {x: 0.5, y: 0.5}
        dock = <Dock> {
            height: Fill,
            width: Fill
                                    
            root = Splitter {
                axis: Horizontal,
                align: FromA(300.0),
                a: tab_set_1,
                b: tab_set_2
            }
            tab_set_1 = Tabs {
                tabs: [tab_a, tab_b],
                selected: 1
            }

            tab_set_2 = Tabs {
                tabs: [],
                selected: 1
            }

            tab_a = Tab {
                name: "Tab A"
                template: PermanentTab,
                kind: Container_A
            }

            tab_b = Tab {
                name: "Tab B"
                template: PermanentTab,
                kind: Container_B
            }

     
            Container_A = <RectView> {
                height: Fill, width: Fill
                padding: 10.,
                <Label> {text: "Hallo"}
                button1 = <Button> { text: "store" }
                button2 = <Button> { text: "load" }
            }
            Container_B = <RectView> {
                height: Fill, width: Fill
                padding: 10.,
                <Label> {text: "Hallo"}
                button3 = <Button> { text: "store" }
                button3 = <Button> { text: "load" }
            }
        }
    }
);

#[derive(Live, LiveHook, Widget)]
pub struct Ui {
    #[deref]
    deref: Window,
}

impl Widget for Ui {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.deref.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        println!("draw_walk");
        self.deref.draw_walk(cx, scope, walk)
    }
}
