use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    
    App = {{App}} {
        ui: <Window> {
            show_bg: true
            draw_bg: {
                fn pixel(self) -> vec4 {
                    return mix(#7, #3, self.pos.y);
                }
            }
            
            body = <View> {
                width: Fill, height: Fill
                flow: Right
                
                left_navigation = <StackNavigation> {
                    width: 300, height: Fill
                    
                    root_view = <View> {
                        width: Fill, height: Fill
                        padding: 20.0
                        spacing: 20.0
                        flow: Down
                        
                        show_bg: true
                        draw_bg: { color: #334 }
                        
                        <Label> {
                            text: "Left Navigation"
                            draw_text: { color: #fff }
                        }
                        
                        <Label> {
                            text: "This is the left stack navigation. Use buttons below to navigate."
                            draw_text: { color: #ccc }
                        }
                        
                        <View> {
                            width: Fill, height: Fit
                            spacing: 10.0
                            flow: Down
                            
                            left_home_button = <Button> {
                                text: "Go to Home"
                                width: 180, height: 40
                            }
                            
                            left_settings_button = <Button> {
                                text: "Go to Settings"
                                width: 180, height: 40
                            }
                            
                            left_profile_button = <Button> {
                                text: "Go to Profile"
                                width: 180, height: 40
                            }
                        }
                    }
                
                    left_home_view = <StackNavigationView> {
                        header = <StackViewHeader> {
                            title = <H4> { text: "Left Home" }
                        }
                        
                        body = <View> {
                            width: Fill, height: Fill
                            padding: 20.0
                            spacing: 20.0
                            flow: Down
                            
                            <Label> {
                                text: "Left Home View"
                                draw_text: { color: #fff}
                            }
                            
                            <Label> {
                                text: "This is the left home stack view."
                                draw_text: { color: #ccc }
                            }
                            
                            <View> {
                                width: Fill, height: Fit
                                spacing: 10.0
                                flow: Down
                                
                                left_go_to_settings = <Button> {
                                    text: "Go to Settings"
                                    width: 180, height: 40
                                }
                                
                                left_go_to_profile = <Button> {
                                    text: "Go to Profile"
                                    width: 180, height: 40
                                }
                                
                                left_back_to_root = <Button> {
                                    text: "Back to Root"
                                    width: 180, height: 40
                                }
                            }
                        }
                    }
                    
                    left_settings_view = <StackNavigationView> {
                        header = <StackViewHeader> {
                            title = <H4> { text: "Left Settings" }
                        }
                        
                        body = <View> {
                            width: Fill, height: Fill
                            padding: 20.0
                            spacing: 20.0
                            flow: Down
                            
                            <Label> {
                                text: "Left Settings"
                                draw_text: { color: #fff}
                            }
                            
                            <Label> {
                                text: "Configure left navigation settings."
                                draw_text: { color: #ccc }
                            }
                            
                            <View> {
                                width: Fill, height: Fit
                                spacing: 10.0
                                flow: Down
                                
                                left_setting1 = <CheckBox> {
                                    text: "Enable notifications"
                                }
                                
                                left_setting2 = <CheckBox> {
                                    text: "Dark mode"
                                }
                                
                                left_setting3 = <CheckBox> {
                                    text: "Auto-save"
                                }
                            }
                        }
                    }
                    
                    left_profile_view = <StackNavigationView> {
                        header = <StackViewHeader> {
                            title = <H4> { text: "Left Profile" }
                        }
                        
                        body = <View> {
                            width: Fill, height: Fill
                            padding: 20.0
                            spacing: 20.0
                            flow: Down
                            
                            <Label> {
                                text: "Left User Profile"
                                draw_text: { color: #fff }
                            }
                            
                            <Label> {
                                text: "Manage left profile information."
                                draw_text: { color: #ccc }
                            }
                            
                            <View> {
                                width: Fill, height: Fit
                                spacing: 15.0
                                flow: Down
                                
                                <TextInput> {
                                    width: 180, height: 30
                                    text: "John Left"
                                    empty_text: "Full Name"
                                }
                                
                                <TextInput> {
                                    width: 180, height: 30
                                    text: "left@example.com"
                                    empty_text: "Email"
                                }
                                
                                left_save_profile = <Button> {
                                    text: "Save Profile"
                                    width: 150, height: 40
                                }
                            }
                        }
                    }
                }
                
                right_navigation = <StackNavigation> {
                    width: 300, height: Fill
                    
                    root_view = <View> {
                        width: Fill, height: Fill
                        padding: 20.0
                        spacing: 20.0
                        flow: Down
                        
                        show_bg: true
                        draw_bg: { color: #443 }
                        
                        <Label> {
                            text: "Right Navigation"
                            draw_text: { color: #fff }
                        }
                        
                        <Label> {
                            text: "This is the right stack navigation. Use buttons below to navigate."
                            draw_text: { color: #ccc }
                        }
                        
                        <View> {
                            width: Fill, height: Fit
                            spacing: 10.0
                            flow: Down
                            
                            right_home_button = <Button> {
                                text: "Go to Home"
                                width: 180, height: 40
                            }
                            
                            right_settings_button = <Button> {
                                text: "Go to Settings"
                                width: 180, height: 40
                            }
                            
                            right_profile_button = <Button> {
                                text: "Go to Profile"
                                width: 180, height: 40
                            }
                        }
                    }
                
                    right_home_view = <StackNavigationView> {
                        header = <StackViewHeader> {
                            title = <H4> { text: "Right Home" }
                        }
                        
                        body = <View> {
                            width: Fill, height: Fill
                            padding: 20.0
                            spacing: 20.0
                            flow: Down
                            
                            <Label> {
                                text: "Right Home View"
                                draw_text: { color: #fff}
                            }
                            
                            <Label> {
                                text: "This is the right home stack view."
                                draw_text: { color: #ccc }
                            }
                            
                            <View> {
                                width: Fill, height: Fit
                                spacing: 10.0
                                flow: Down
                                
                                right_go_to_settings = <Button> {
                                    text: "Go to Settings"
                                    width: 180, height: 40
                                }
                                
                                right_go_to_profile = <Button> {
                                    text: "Go to Profile"
                                    width: 180, height: 40
                                }
                                
                                right_back_to_root = <Button> {
                                    text: "Back to Root"
                                    width: 180, height: 40
                                }
                            }
                        }
                    }
                    
                    right_settings_view = <StackNavigationView> {
                        header = <StackViewHeader> {
                            title = <H4> { text: "Right Settings" }
                        }
                        
                        body = <View> {
                            width: Fill, height: Fill
                            padding: 20.0
                            spacing: 20.0
                            flow: Down
                            
                            <Label> {
                                text: "Right Settings"
                                draw_text: { color: #fff}
                            }
                            
                            <Label> {
                                text: "Configure right navigation settings."
                                draw_text: { color: #ccc }
                            }
                            
                            <View> {
                                width: Fill, height: Fit
                                spacing: 10.0
                                flow: Down
                                
                                right_setting1 = <CheckBox> {
                                    text: "Advanced features"
                                }
                                
                                right_setting2 = <CheckBox> {
                                    text: "Auto-sync"
                                }
                                
                                right_setting3 = <CheckBox> {
                                    text: "Beta updates"
                                }
                            }
                        }
                    }
                    
                    right_profile_view = <StackNavigationView> {
                        header = <StackViewHeader> {
                            title = <H4> { text: "Right Profile" }
                        }
                        
                        body = <View> {
                            width: Fill, height: Fill
                            padding: 20.0
                            spacing: 20.0
                            flow: Down
                            
                            <Label> {
                                text: "Right User Profile"
                                draw_text: { color: #fff }
                            }
                            
                            <Label> {
                                text: "Manage right profile information."
                                draw_text: { color: #ccc }
                            }
                            
                            <View> {
                                width: Fill, height: Fit
                                spacing: 15.0
                                flow: Down
                                
                                <TextInput> {
                                    width: 180, height: 30
                                    text: "Jane Right"
                                    empty_text: "Full Name"
                                }
                                
                                <TextInput> {
                                    width: 180, height: 30
                                    text: "right@example.com"
                                    empty_text: "Email"
                                }
                                
                                right_save_profile = <Button> {
                                    text: "Save Profile"
                                    width: 150, height: 40
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Live, LiveHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
}

impl LiveRegister for App {
    fn live_register(cx: &mut Cx) {
        crate::makepad_widgets::live_design(cx);
    }
}

impl MatchEvent for App {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let navigation = self.ui.stack_navigation(id!(left_navigation));
        
        // Handle root view navigation
        if self.ui.button(id!(left_home_button)).clicked(&actions) {
            navigation.push(cx, live_id!(left_home_view));
        }
        if self.ui.button(id!(left_settings_button)).clicked(&actions) {
            navigation.push(cx, live_id!(left_settings_view));
        }
        if self.ui.button(id!(left_profile_button)).clicked(&actions) {
            navigation.push(cx, live_id!(left_profile_view));
        }
        
        // Handle home view navigation
        if self.ui.button(id!(left_go_to_settings)).clicked(&actions) {
            navigation.push(cx, live_id!(left_settings_view));
        }
        if self.ui.button(id!(left_go_to_profile)).clicked(&actions) {
            navigation.push(cx, live_id!(left_profile_view));
        }
        if self.ui.button(id!(left_back_to_root)).clicked(&actions) {
            navigation.pop_to_root(cx);
        }
        
        // Handle settings view navigation
        if self.ui.button(id!(left_advanced_settings)).clicked(&actions) {
            navigation.push(cx, live_id!(left_advanced_settings_view));
        }
        
        // Handle profile save
        if self.ui.button(id!(left_save_profile)).clicked(&actions) {
            log!("Profile saved!");
        }
        
        // Let the navigation handle its own actions
        navigation.handle_stack_view_actions(cx, actions);

        let right_navigation = self.ui.stack_navigation(id!(right_navigation));
        // Handle root view right navigation
        if self.ui.button(id!(right_home_button)).clicked(&actions) {
            right_navigation.push(cx, live_id!(right_home_view));
        }
        if self.ui.button(id!(right_settings_button)).clicked(&actions) {
            right_navigation.push(cx, live_id!(right_settings_view));
        }
        if self.ui.button(id!(right_profile_button)).clicked(&actions) {
            right_navigation.push(cx, live_id!(right_profile_view));
        }
        right_navigation.handle_stack_view_actions(cx, actions);
    }
}

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}

app_main!(App);