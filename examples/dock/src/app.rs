
use std::collections::HashMap;

use makepad_widgets::*;
use std::sync::atomic::Ordering;
live_design!{
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    PermanentTab = <Tab> {closeable:false}

    App = {{App}} {
        ui: <Root>{
            main_window = <Window>{
                button1 = <Button> {
                    text: "save"
                    draw_text:{color:#fff}
                }
                button2 = <Button> {
                    text: "load"
                    draw_text:{color:#fff}
                }
                body = <View>{
                    flow: Down,
                    spacing: 10,
                    align: {
                        x: 0.5,
                        y: 0.5
                    },
                    
                    dock = <Dock> {

                        width: Fill,
                        height: Fill,
                        padding: 100,
                        spacing: 0,
            
                        root = Splitter {
                            axis: Horizontal,
                            align: FromA(300.0),
                            //a: rooms_sidebar_tab,
                            a: main
                        }
                        // Not really a tab, but it needs to be one to be used in the dock
                        rooms_sidebar_tab = Tab {
                            name: "" // show no tab header
                            kind: welcome_screen
                        }
                        main = Tabs{tabs:[home_tab, home_tab2, home_tab3], selected:0}
            
                        home_tab = Tab {
                            name: "Home"
                            kind: welcome_screen
                            template: PermanentTab
                        }
                        home_tab2 = Tab {
                            name: "Home2"
                            kind: welcome_screen
                            template: PermanentTab
                        }
                        home_tab3 = Tab {
                            name: "Home3"
                            kind: welcome_screen
                            template: PermanentTab
                        }
                        welcome_screen = <View> {}
                    }
                }
            }
        }
    }
}  

app_main!(App); 
 
#[derive(Live, LiveHook)]
pub struct App {
    #[live] ui: WidgetRef,
    #[rust] counter: usize,
    #[rust] dock_state: HashMap<LiveId,DockItem>
 }
 
impl LiveRegister for App {
    fn live_register(cx: &mut Cx) {
        crate::makepad_widgets::live_design(cx);
    }
}

impl MatchEvent for App{

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(id!(button1)).clicked(&actions) {
            let dock = self.ui.dock(id!(dock));
            let h = dock.clone_state();
            
         
            if let Some(dock_items) = h {
                self.dock_state = dock_items;
            }
        }
        if self.ui.button(id!(button2)).clicked(&actions) {
            let dock = self.ui.dock(id!(dock));
            let Some(mut dock) = dock.borrow_mut() else {return };
            UNIQUE_LIVE_ID.store(1, Ordering::SeqCst);
            dock.load_state(cx, self.dock_state.clone());
        }
        for action in actions{
            let dock: DockRef = self.ui.dock(id!(dock));

            if let Some(action) = action.as_widget_action() {
                // Handle Dock actions
                let mut should_save_dock_action: bool = false;
                match action.cast() {
                    // Whenever a tab (except for the home_tab) is pressed, notify the app state.
                    DockAction::TabWasPressed(tab_id) => {
                        println!("pressed");
                    }
                    DockAction::TabCloseWasPressed(tab_id) => {
                        
                    }
                    // When dragging a tab, allow it to be dragged
                    DockAction::ShouldTabStartDrag(tab_id) => {
                        dock.tab_start_drag(
                            cx,
                            tab_id,
                            DragItem::FilePath {
                                path: "".to_string(),
                                internal_id: Some(tab_id),
                            },
                        );
                    }
                    // When dragging a tab, allow it to be dragged
                    DockAction::Drag(drag_event) => {
                        if drag_event.items.len() == 1 {
                            dock.accept_drag(cx, drag_event, DragResponse::Move);
                        }
                    }
                    // When dropping a tab, move it to the new position
                    DockAction::Drop(drop_event) => {
                        // from inside the dock, otherwise it's an external file
                        if let DragItem::FilePath {
                            internal_id: Some(internal_id),
                            ..
                        } = &drop_event.items[0] {
                            dock.drop_move(cx, drop_event.abs, *internal_id);
                        }
                    }
                    DockAction::SplitPanelChanged { panel_id: _, axis: _, align: _ } => {
                    
                    }
                    _ => (),
                }
            }
        }
    }
    // fn handle_action(&mut self, cx: &mut Cx, action: &Action) {
    //     let dock = self.ui.dock(id!(dock));
        
    //     if let Some(action) = action.as_widget_action() {
    //         // Handle Dock actions
    //         let mut should_save_dock_action: bool = false;
    //         match action.cast() {
    //             // Whenever a tab (except for the home_tab) is pressed, notify the app state.
    //             DockAction::TabWasPressed(tab_id) => {
    //                 println!("pressed");
    //             }
    //             DockAction::TabCloseWasPressed(tab_id) => {
                    
    //             }
    //             // When dragging a tab, allow it to be dragged
    //             DockAction::ShouldTabStartDrag(tab_id) => {
    //                 dock.tab_start_drag(
    //                     cx,
    //                     tab_id,
    //                     DragItem::FilePath {
    //                         path: "".to_string(),
    //                         internal_id: Some(tab_id),
    //                     },
    //                 );
    //             }
    //             // When dragging a tab, allow it to be dragged
    //             DockAction::Drag(drag_event) => {
    //                 if drag_event.items.len() == 1 {
    //                     dock.accept_drag(cx, drag_event, DragResponse::Move);
    //                 }
    //             }
    //             // When dropping a tab, move it to the new position
    //             DockAction::Drop(drop_event) => {
    //                 // from inside the dock, otherwise it's an external file
    //                 if let DragItem::FilePath {
    //                     internal_id: Some(internal_id),
    //                     ..
    //                 } = &drop_event.items[0] {
    //                     dock.drop_move(cx, drop_event.abs, *internal_id);
    //                 }
    //             }
    //             DockAction::SplitPanelChanged { panel_id: _, axis: _, align: _ } => {
                   
    //             }
    //             _ => (),
    //         }
    //     }
    // }
}

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}