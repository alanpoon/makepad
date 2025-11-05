
use makepad_widgets::{image_cache::ImageCacheImpl, *};

use crate::image_viewer::{ImageViewerAction, ImageViewerWidgetRefExt};

live_design!{
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::image_viewer::ImageViewer;
    App = {{App}} {
        ui: <Root>{
            main_window = <Window>{
                window: {title: "你好，こんにちは, Привет, Hello"},
                body = <View> {
                    width: Fill,
                    height: Fill
                    flow: Overlay
                    <View> {
                        width: Fill,
                        height: Fill,
                        padding: 0,
                        flow: Down
                        <View> {
                            width: 300,
                            height: 200,
                            button_1 = <Button> {
                                margin: 0.0,
                                width: 100,
                                height: 100,
                                descender: 50,
                            }
                        }
                        <View> {
                            width: Fill,
                            height: Fill
                            underlying_image = <Image>{
                                source: dep("crate://self/resources/ducky.png" ),
                                fit: Smallest
                                width: 500,
                                height: 500
                            }
                        }                       
                    }
                    image_viewer_modal = <Modal> {
                        content: {
                            width: Fill, height: Fill,
                            flow: Down
                            label1 = <Label> {
                                draw_text: {
                                    color: #fff
                                },
                                text: "Counter: 0"
                            }
                            image_viewer_inner = <ImageViewer> {
                                align: {x: 0.5, y: 0.5}
                            }
                        }
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
        crate::image_viewer::live_design(cx);
    }
}

impl MatchEvent for App{
    fn handle_startup(&mut self, _cx:&mut Cx){
    }
        
    fn handle_actions(&mut self, cx: &mut Cx, actions:&Actions){
        if self.ui.button(ids!(button_1)).clicked(&actions) {
            self.ui.button(ids!(button_1)).set_text(cx, "Clicked 😀");
            log!("hi --- ");
            //self.ui.modal(ids!(image_modal)).open(cx);
            let texture = self.ui.image(ids!(underlying_image))
                .borrow()
                .and_then(|f|f.get_texture(0).clone());
            let size = self.ui.image(ids!(underlying_image)).area().rect(cx).size;
            println!("underlying_image size {:?} texture {:?}", size, texture.is_some());
            self.ui.modal(ids!(image_viewer_modal)).open(cx);
            
            self.ui.image_viewer(ids!(image_viewer_inner)).display_using_texture(cx, texture, &size);
            self.counter += 1;
        }
        for action in actions {
            if let Some(ImageViewerAction::Hide) = action.downcast_ref() {
                self.ui.modal(ids!(image_viewer_modal)).close(cx);
            }
        }
    }
}

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        if let Event::XrUpdate(_e) = event{
            //log!("{:?}", e.now.left.trigger.analog);
        }
        let underlying_image = self.ui.image(ids!(underlying_image));
        match event.hits(cx, underlying_image.area()) {
            Hit::FingerDown(fd) => {
                println!("FingerDown: {}", fd.is_primary_hit());
            }
            _ => {}
        }
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}