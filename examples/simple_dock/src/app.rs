
use makepad_widgets::*;
use std::{collections::HashMap, time::Duration};
live_design!{
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::ui::*;

    App = {{App}} {
        ui: <Ui> {}
    }
}

app_main!(App); 
 
#[derive(Live, LiveHook)]
pub struct App {
    #[live] ui: WidgetRef,
    #[rust] counter: usize,
    #[rust]
    pub dock: Option<HashMap<LiveId, DockItem>>,
 }
 
impl LiveRegister for App {
    fn live_register(cx: &mut Cx) {
        crate::makepad_widgets::live_design(cx);
        crate::ui::live_design(cx);
    }
}

impl MatchEvent for App{
    fn handle_actions(&mut self, cx: &mut Cx, actions:&Actions){
        if self.ui.button(id!(button1)).clicked(&actions) {
            self.counter += 1;
            println!("clicked");
            let dock = self.ui.dock(id!(dock));
            if let Some(dock_state) = dock.clone_state() {
                self.dock = Some(dock_state);
            }
        }
        if self.ui.button(id!(button2)).clicked(&actions) {
            println!("clicked2");
            let dock = self.ui.dock(id!(dock));
            
            if let Some(ref dock_state) = self.dock {
                let Some(mut dock) = dock.borrow_mut() else {return };
                println!("window_geom_change_event {:?}", dock_state);
                //dock.load_state(cx, dock_state.clone());
                std::thread::sleep(Duration::from_millis(3000));
                println!("done_load_state");
                self.ui.clear_query_cache();
            }
        }
    }
}

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
