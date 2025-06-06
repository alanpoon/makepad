
use makepad_widgets::*;

use crate::popup_notification::RobrixPopupNotificationWidgetRefExt;

live_design!{
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::popup_notification::*;

    App = {{App}} {
        ui: <Root>{
            main_window = <Window>{
                body = <View>{
                    flow: Down,
                    spacing: 10,
                    align: {
                        x: 0.5,
                        y: 0.5
                    },
                    show_bg: true,
                    draw_bg:{
                        fn pixel(self) -> vec4 {
                            let center = vec2(0.5, 0.5);
                            let uv = self.pos - center;
                            let radius = length(uv);
                            let angle = atan(uv.y, uv.x);
                            let color1 = mix(#f00, #00f, 0.5 + 10.5 * cos(angle + self.time));
                            let color2 = mix(#0f0, #ff0, 0.5 + 0.5 * sin(angle + self.time));
                            return mix(color1, color2, radius);
                        }
                    }
                    button_1 = <Button> {
                        text: "Click me 123"
                        draw_text:{color:#fff}
                    }
                    text_input = <TextInput> {
                        width: 100,
                        flow: RightWrap,
                        text: "Lorem ipsum"
                        draw_text:{color:#fff, }
                    }
                    button_2 = <Button> {
                        text: "Click me 345"
                        draw_text:{color:#fff}
                    }
                    popup = <RobrixPopupNotification>{
                        duration: 1.0
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
 }
 
impl LiveRegister for App {
    fn live_register(cx: &mut Cx) { 
        crate::makepad_widgets::live_design(cx);
        crate::popup_notification::live_design(cx);
    }
}

impl MatchEvent for App{
    fn handle_startup(&mut self, cx:&mut Cx){
    }
    
    fn handle_timer(&mut self, _cx:&mut Cx, _te:&TimerEvent){
    }
    
    fn handle_actions(&mut self, cx: &mut Cx, actions:&Actions){
        if self.ui.button(id!(button_1)).clicked(&actions) {
            println!("click");
            self.counter += 1;
            let counter_text = format!("count {:?} asdasdasdasdasd d asdasd ad asd ad ad ada d asd ad ad asd d sd",self.counter );
            self.ui.robrix_popup_notification(id!(popup)).open(cx, Box::new(move|label_ref,cx |label_ref.set_text(cx,&counter_text)));
        }
        
    }
}

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}