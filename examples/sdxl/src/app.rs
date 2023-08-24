use crate::{makepad_live_id::*};
use makepad_micro_serde::*;
use makepad_widgets::*;
use std::fs;
use std::time::Instant;
use crate::database::*;
use crate::comfyui::*;

live_design!{
    import makepad_widgets::button::Button;
    import makepad_widgets::desktop_window::DesktopWindow;
    import makepad_widgets::label::Label;
    import makepad_widgets::image::Image;
    import makepad_widgets::text_input::TextInput;
    import makepad_widgets::image::Image;
    import makepad_widgets::list_view::ListView;
    import makepad_widgets::drop_down::DropDown;
    import makepad_widgets::slide_panel::SlidePanel;
    import makepad_widgets::frame::*;
    import makepad_widgets::theme::*;
    import makepad_draw::shader::std::*;
    import makepad_widgets::dock::*;
    
    
    TEXT_BIG = 12.0
    
    TEXT_BOLD = {
        font_size: 10.0,
        font: {path: dep("crate://makepad-widgets/resources/IBMPlexSans-SemiBold.ttf")}
    }
    COLOR_UP_0 = #xFFFFFF00
    COLOR_DOWN_2 = #x00000022
    FONT_SIZE_H2 = 10.0
    
    SSPACING_0 = 0.0
    SSPACING_1 = 4.0
    SSPACING_2 = (SSPACING_1 * 2)
    SSPACING_3 = (SSPACING_1 * 3)
    SSPACING_4 = (SSPACING_1 * 4)
    
    SPACING_0 = {top: (SSPACING_0), right: (SSPACING_0), bottom: (SSPACING_0), left: (SSPACING_0)}
    SPACING_1 = {top: (SSPACING_1), right: (SSPACING_1), bottom: (SSPACING_1), left: (SSPACING_1)}
    SPACING_2 = {top: (SSPACING_2), right: (SSPACING_2), bottom: (SSPACING_2), left: (SSPACING_2)}
    SPACING_3 = {top: (SSPACING_3), right: (SSPACING_3), bottom: (SSPACING_3), left: (SSPACING_3)}
    SPACING_4 = {top: (SSPACING_4), right: (SSPACING_4), bottom: (SSPACING_4), left: (SSPACING_4)}
    H2_TEXT_BOLD = {
        font_size: (FONT_SIZE_H2),
        font: {path: dep("crate://makepad-widgets/resources/IBMPlexSans-SemiBold.ttf")}
    }
    H2_TEXT_REGULAR = {
        font_size: (FONT_SIZE_H2),
        font: {path: dep("crate://makepad-widgets/resources/IBMPlexSans-Text.ttf")}
    }
    
    COLOR_PANEL_BG = (COLOR_DOWN_2)
    COLOR_TEXT_INPUT = (COLOR_DOWN_2)
    
    
    SdxlDropDown = <DropDown> {
        walk: {width: Fit}
        layout: {padding: {top: (SSPACING_2), right: (SSPACING_4), bottom: (SSPACING_2), left: (SSPACING_2)}}
        
        draw_label: {
            text_style: <H2_TEXT_REGULAR> {},
            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        mix(
                            (#xFFF8),
                            (#xFFF8),
                            self.focus
                        ),
                        (#xFFFF),
                        self.hover
                    ),
                    (#x000A),
                    self.pressed
                )
            }
        }
        
        popup_menu: {
            menu_item: {
                indent_width: 10.0
                walk: {width: Fill, height: Fit}
                
                layout: {
                    padding: {left: (SSPACING_4), top: (SSPACING_2), bottom: (SSPACING_2), right: (SSPACING_4)},
                }
                
                draw_bg: {
                    color: #x48,
                    color_selected: #x6
                }
            }
        }
        
        draw_bg: {
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                self.get_bg(sdf);
                // triangle
                let c = vec2(self.rect_size.x - 10.0, self.rect_size.y * 0.5)
                let sz = 2.5;
                
                sdf.move_to(c.x - sz, c.y - sz);
                sdf.line_to(c.x + sz, c.y - sz);
                sdf.line_to(c.x, c.y + sz * 0.75);
                sdf.close_path();
                
                sdf.fill(mix(#FFFA, #FFFF, self.hover));
                
                return sdf.result
            }
            
            fn get_bg(self, inout sdf: Sdf2d) {
                sdf.rect(
                    0,
                    0,
                    self.rect_size.x,
                    self.rect_size.y
                )
                sdf.fill((COLOR_UP_0))
            }
        }
    }
    
    DividerV = <Frame> {
        layout: {flow: Down, spacing: 0.0}
        walk: {margin: {top: 0.0, right: 0.0, bottom: 10.0, left: 0.0}, width: Fill, height: Fit}
        <Rect> {
            walk: {height: 2, width: Fill, margin: 0.0}
            layout: {flow: Down, padding: 0.0},
            draw_bg: {color: #x00000066}
        }
        <Rect> {
            walk: {height: 2, width: Fill, margin: 0.0}
            layout: {flow: Down, padding: 0.0},
            draw_bg: {color: #xFFFFFF22}
        }
    }
    
    ProgressCircle = <Frame> {
        show_bg: true,
        walk: {width: 25, height: 25}
        draw_bg: {
            instance progress: 0.0
            instance active: 0.0
            
            fn circle_pie(inout sdf: Sdf2d, x: float, y: float, r: float, s: float) {
                let c = sdf.pos - vec2(x, y);
                let len = sqrt(c.x * c.x + c.y * c.y) - r;
                let pi = 3.141592653589793;
                let ang = (pi - atan(c.x, c.y)) / (2.0 * pi);
                let ces = s * 0.5;
                let ang2 = clamp((abs(ang - ces) - ces) * -r * r * sdf.scale_factor, 0.0, 1.0);
                sdf.dist = len * ang2 / sdf.scale_factor;
                sdf.old_shape = sdf.shape;
                sdf.shape = min(sdf.shape, sdf.dist);
            }
            
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                sdf.circle(
                    self.rect_size.x * 0.5,
                    self.rect_size.y * 0.5,
                    self.rect_size.x * 0.4
                );
                sdf.fill(mix(#4, #575, self.active));
                circle_pie(
                    sdf,
                    self.rect_size.x * 0.5,
                    self.rect_size.y * 0.5,
                    self.rect_size.x * 0.4,
                    self.progress
                );
                sdf.fill(mix(#4, #8f8, self.active));
                return sdf.result;
            }
        }
    }
    /*
    PromptGroup = <Rect> {
        draw_bg:{
            instance hover: 0.0
            instance down: 0.0
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                sdf.box(1, 1, self.rect_size.x - 2, self.rect_size.y - 2, 4.0)
                sdf.fill(mix( mix(#x00000000, #x00000000, self.hover), #x00000000, self.down));
                return sdf.result
            }
        }
        walk: {height: Fit, width: Fill, margin: {bottom: 10, top: 0, left: 0, right: 0 } }
        layout: {flow: Right, spacing: 10,  padding: 0}
        state: {
            hover = {
                default: off,
                off = {
                    from: {all: Forward {duration: 0.5}}
                    ease: OutExp
                    apply: {
                        draw_bg: {hover: 0.0}
                     }
                }
                on = {
                    ease: OutExp
                    from: {
                        all: Forward {duration: 0.2}
                    }
                    apply: {
                        draw_bg: {hover: 1.0}
                     }
                }
            }
            down = {
                default: off
                off = {
                    from: {all: Forward {duration: 0.5}}
                    ease: OutExp
                    apply: {
                        draw_bg: {down: 0.0}
                     }
                }
                on = {
                    ease: OutExp
                    from: {
                        all: Forward {duration: 0.2}
                    }
                    apply: {
                       draw_bg: {down: 1.0}
                     }
                }
            }
        }


        <Frame> {
            layout: {flow: Down},
            walk: { width: Fill, height: Fit}

            <DividerV> {}

            prompt = <Button> {
                walk: { width: Fill, height:Fit }
                layout: { align: {x: 0.0, y: 0.}, padding: {top: 5.0, right: 0.0, bottom: 5.0, left: 0.0} }
                draw_bg: {
                    fn pixel(self) -> vec4 {
                        let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                        let body = mix(mix(#53, #5c, self.hover), #33, self.pressed);
                        sdf.fill_keep(body)
                        return sdf.result
                    }
                }
                draw_label: {
                    walk: { width: Fill }
                    text_style: <TEXT_BOLD> {},
                    fn get_color(self) -> vec4 {
                        return mix(mix(#xFFFA, #xFFFF, self.hover), #xFFF8, self.pressed);
                    }
                    wrap: Word,
                }
                label: "Placeholder Lorem Ipsum dolor Sit amet"
            }
        }
    }*/
    
    PromptGroup = <Rect> {
        <DividerV> {}
        draw_bg: {
            instance hover: 0.0
            instance down: 0.0
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                let body = mix(mix(#53, #5c, self.hover), #33, self.down);
                sdf.fill_keep(body)
                return sdf.result
            }
        }
        walk: {height: Fit, width: Fill, margin: {bottom: 10, top: 20}}
        layout: {flow: Down, spacing: 0, padding: 10}
        state: {
            hover = {
                default: off,
                off = {
                    from: {all: Forward {duration: 0.5}}
                    ease: OutExp
                    apply: {
                        draw_bg: {hover: 0.0}
                        prompt = {draw_label: {hover: 0.0}}
                    }
                }
                on = {
                    ease: OutExp
                    from: {
                        all: Forward {duration: 0.2}
                    }
                    apply: {
                        draw_bg: {hover: 1.0}
                        prompt = {draw_label: {hover: 1.0}}
                    }
                }
            }
            down = {
                default: off
                off = {
                    from: {all: Forward {duration: 0.5}}
                    ease: OutExp
                    apply: {
                        draw_bg: {down: 0.0}
                        prompt = {draw_label: {down: 0.0}}
                    }
                }
                on = {
                    ease: OutExp
                    from: {
                        all: Forward {duration: 0.2}
                    }
                    apply: {
                        draw_bg: {down: 1.0}
                        prompt = {draw_label: {down: 1.0}}
                    }
                }
            }
        }
        prompt = <Label> {
            walk: {width: Fill}
            draw_label: {
                text_style: <TEXT_BOLD> {},
                instance hover: 0.0
                instance down: 0.0
                fn get_color(self) -> vec4 {
                    return mix(mix(#xFFFA, #xFFFF, self.hover), #xFFF8, self.down);
                }
                wrap: Word,
            }
            label: ""
        }
    }
    
    ImageTile = <Frame> {
        walk: {width: Fill, height: Fit},
        cursor: Hand
        state: {
            hover = {
                default: off,
                off = {
                    from: {all: Forward {duration: 0.5}}
                    ease: OutExp
                    apply: {
                        img = {draw_bg: {hover: 0.0}}
                    }
                }
                on = {
                    ease: OutExp
                    from: {
                        all: Forward {duration: 0.2}
                    }
                    apply: {
                        img = {draw_bg: {hover: 1.0}}
                    }
                }
            }
            down = {
                default: off
                off = {
                    from: {all: Forward {duration: 0.5}}
                    ease: OutExp
                    apply: {
                        img = {draw_bg: {down: 0.0}}
                    }
                }
                on = {
                    ease: OutExp
                    from: {
                        all: Forward {duration: 0.2}
                    }
                    apply: {
                        img = {draw_bg: {down: 1.0}}
                    }
                }
            }
        }
        
        img = <Image> {
            walk: {width: Fill, height: Fill}
            fit: Horizontal,
            draw_bg: {
                instance hover: 0.0
                instance down: 0.0
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                    sdf.box(1, 1, self.rect_size.x - 2, self.rect_size.y - 2, 4.0)
                    let max_scale = vec2(0.97);
                    let scale = mix(vec2(1.0), max_scale, self.hover);
                    let pan = mix(vec2(0.0), (vec2(1.0) - max_scale) * 0.5, self.hover);
                    let color = self.get_color(scale, pan) + mix(vec4(0.0), vec4(0.1), self.down);
                    sdf.fill(color);
                    return sdf.result
                }
            }
        }
    }
    
    App = {{App}} {
        last_seed: 1000;
        ui: <DesktopWindow> {
            window: {inner_size: vec2(2000, 1024)},
            caption_bar = {visible: true, caption_label = {label = {label: "SDXL Surf"}}},
            
            <Frame> {
                layout: {
                    flow: Overlay,
                },
                walk: {
                    width: Fill,
                    height: Fill
                },
                
                dock = <Dock> {
                    walk: {height: Fill, width: Fill}
                    
                    root = Splitter {
                        axis: Horizontal,
                        align: FromA(300.0),
                        a: image_library,
                        b: split1
                    }
                    
                    split1 = Splitter {
                        axis: Vertical,
                        align: FromB(200.0),
                        a: image_view,
                        b: input_panel
                    }
                    
                    image_library = Tab {
                        name: ""
                        kind: ImageLibrary
                    }
                    
                    input_panel = Tab {
                        name: ""
                        kind: InputPanel
                    }
                    
                    image_view = Tab {
                        name: ""
                        kind: ImageView
                    }
                    
                    ImageView = <Rect> {
                        draw_bg: {color: (COLOR_PANEL_BG)}
                        walk: {height: Fill, width: Fill}
                        layout: {flow: Down, align: {x: 0.5, y: 0.5}}
                        cursor: Hand,
                        image = <Image> {
                            fit: Smallest,
                            walk: {width: Fill, height: Fill}
                        }
                    }
                    
                    InputPanel = <Rect> {
                        walk: {height: Fill, width: Fill}
                        layout: {flow: Down, padding: 5}
                        draw_bg: {color: (COLOR_PANEL_BG)}
                        <Frame> {
                            walk: {height: Fit, width: Fill}
                            layout: {align: {x: 1.0, y: 0.5}}
                            
                            <Label> {
                                walk: {margin: {left: 10}},
                                label: "Workflow",
                                draw_label: {
                                    text_style: <TEXT_BOLD> {},
                                    fn get_color(self) -> vec4 {
                                        return #CCCCCC
                                    }
                                }
                            }
                            workflow_dropdown = <SdxlDropDown> {}
                            <Label> {
                                walk: {margin: {left: 10}},
                                label: "Batch",
                                draw_label: {
                                    text_style: <TEXT_BOLD> {},
                                    fn get_color(self) -> vec4 {
                                        return #CCCCCC
                                    }
                                }
                            }
                            batch_mode_dropdown = <SdxlDropDown> {
                                selected_item: 0
                                labels: ["1", "2", "3", "4", "5", "6", "stepped"]
                            }
                            
                            progress1 = <ProgressCircle> {}
                            progress2 = <ProgressCircle> {}
                            progress3 = <ProgressCircle> {}
                            progress4 = <ProgressCircle> {}
                            progress5 = <ProgressCircle> {}
                            progress6 = <ProgressCircle> {
                                walk: {margin: {right: 5.0}}
                            }
                            
                            render = <Button> {
                                layout: {padding: {top: 5.0, right: 7.5, bottom: 5.0, left: 7.5}}
                                walk: {margin: {top: 5.0, right: 5.0, bottom: 5.0, left: 5.0}}
                                label: "Render"
                                draw_label: {
                                    text_style: <TEXT_BOLD> {},
                                }
                            }
                            
                        }
                        <Frame> {
                            positive = <TextInput> {
                                walk: {width: Fill, height: Fill, margin: {top: 0.0, right: 5.0, bottom: 5.0, left: 5.0}},
                                text: "Positive"
                                draw_label: {text_style: {font_size: (TEXT_BIG)}}
                                draw_bg: {
                                    color: (COLOR_TEXT_INPUT)
                                    border_width: 1.0
                                    border_color: #x00000044
                                }
                            }
                            negative = <TextInput> {
                                walk: {width: Fill, height: Fill, margin: {top: 0.0, left: 0.0, bottom: 5.0, right: 5.0}},
                                draw_label: {text_style: {font_size: (TEXT_BIG)}}
                                text: "text, watermark, cartoon"
                                draw_bg: {
                                    color: (COLOR_TEXT_INPUT)
                                    border_width: 1.0
                                    border_color: #x00000044
                                }
                            }
                        }
                    }
                    
                    ImageLibrary = <Rect> {
                        draw_bg: {color: (COLOR_PANEL_BG)}
                        walk: {height: Fill, width: Fill}
                        layout: {flow: Down},
                        <Frame> {
                            walk: {height: Fit, width: Fill}
                            layout: {flow: Right, padding: {left: 10, right: 10, top: 10}},
                            search = <TextInput> {
                                walk: {height: Fit, width: Fill}
                                empty_message: "Search"
                                draw_bg: {
                                    color: (COLOR_TEXT_INPUT)
                                    border_width: 1.0
                                    border_color: #x00000044
                                }
                                draw_label: {
                                    text_style: {font_size: (TEXT_BIG)}
                                    fn get_color(self) -> vec4 {
                                        return
                                        mix(
                                            mix(
                                                mix(
                                                    #xFFFFFF55,
                                                    #xFFFFFF88,
                                                    self.hover
                                                ),
                                                #xFFFFFFCC,
                                                self.focus
                                            ),
                                            #xFFFFFF66,
                                            self.is_empty
                                        )
                                    }
                                }
                            }
                        }
                        image_list = <ListView> {
                            walk: {height: Fill, width: Fill}
                            layout: {flow: Down, padding: 10}
                            
                            PromptGroup = <PromptGroup> {}
                            
                            Empty = <Frame> {}
                            
                            ImageRow1 = <Frame> {
                                walk: {height: Fit, width: Fill, margin: {bottom: 10}}
                                layout: {spacing: 20, flow: Right},
                                row1 = <ImageTile> {}
                            }
                            ImageRow2 = <Frame> {
                                walk: {height: Fit, width: Fill, margin: {bottom: 10}}
                                layout: {spacing: 20, flow: Right},
                                row1 = <ImageTile> {}
                                row2 = <ImageTile> {}
                            }
                            ImageRow3 = <Frame> {
                                walk: {height: Fit, width: Fill, margin: {bottom: 10}}
                                layout: {spacing: 20, flow: Right},
                                row1 = <ImageTile> {}
                                row2 = <ImageTile> {}
                                row3 = <ImageTile> {}
                            }
                        }
                    }
                }
                
                big_image = <Rect> {
                    visible: false,
                    draw_bg: {draw_depth: 10.0}
                    draw_bg: {color: #0}
                    walk: {height: All, width: All, abs_pos: vec2(0.0, 0.0)}
                    layout: {flow: Down, align: {x: 0.5, y: 0.5}}
                    cursor: Hand,
                    image = <Image> {
                        draw_bg: {draw_depth: 11.0}
                        fit: Smallest,
                        walk: {width: Fill, height: Fill}
                    }
                }
            }
        }
    }
}

app_main!(App);

struct Machine {
    ip: String,
    id: LiveId,
    running: Option<RunningPrompt>,
    fetching: Option<RunningPrompt>
}

struct RunningPrompt {
    _started: Instant,
    steps_counter: usize,
    prompt_state: PromptState,
}

impl Machine {
    fn new(ip: &str, id: LiveId) -> Self {Self {
        ip: ip.to_string(),
        id,
        running: None,
        fetching: None
    }}
}

struct Workflow {
    name: String,
    total_steps: usize
}
impl Workflow {
    fn new(name: &str, total_steps: usize) -> Self {Self {name: name.to_string(), total_steps}}
}


#[derive(Live)]
pub struct App {
    #[live] ui: WidgetRef,
    #[rust(vec![
        Machine::new("192.168.1.62:8188", id_lut!(m1)),
        Machine::new("192.168.1.204:8188", id_lut!(m2)),
        Machine::new("192.168.1.154:8188", id_lut!(m3)),
        Machine::new("192.168.1.144:8188", id_lut!(m4)),
        Machine::new("192.168.1.59:8188", id_lut!(m5)),
        Machine::new("192.168.1.180:8188", id_lut!(m6))
    ])] machines: Vec<Machine>,
    
    #[rust(vec![
        Workflow::new("1024", 25),
        Workflow::new("2048", 40),
        Workflow::new("3000", 37),
        Workflow::new("3840", 276)
    ])] workflows: Vec<Workflow>,
    
    #[rust] queue: Vec<PromptState>,
    
    #[rust(Database::new(cx))] db: Database,
    
    #[rust] filtered: FilteredDb,
    #[live] last_seed: i64,
    
    #[rust] current_image: Option<ImageId>
}

impl LiveHook for App {
    fn before_live_design(cx: &mut Cx) {
        crate::makepad_widgets::live_design(cx);
    }
    
    fn after_new_from_doc(&mut self, cx: &mut Cx) {
        self.open_web_socket(cx);
        let _ = self.db.load_database();
        self.filtered.filter_db(&self.db, "", false);
        let workflows = self.workflows.iter().map( | v | v.name.clone()).collect();
        let dd = self.ui.get_drop_down(id!(workflow_dropdown));
        dd.set_labels(workflows);
    }
}

impl App {
    fn send_prompt(&mut self, cx: &mut Cx, prompt_state: PromptState) {
        // lets find a machine with the minimum queue size and
        for machine in &mut self.machines {
            if machine.running.is_some() {
                continue
            }
            let url = format!("http://{}/prompt", machine.ip);
            let mut request = HttpRequest::new(url, HttpMethod::POST);
            
            request.set_header("Content-Type".to_string(), "application/json".to_string());
            
            let ws = fs::read_to_string(format!("examples/sdxl/workspace_{}.json", prompt_state.workflow)).unwrap();
            let ws = ws.replace("CLIENT_ID", "1234");
            let ws = ws.replace("TEXT_INPUT", &prompt_state.prompt.positive.replace("\n", "").replace("\"", ""));
            let ws = ws.replace("KEYWORD_INPUT", &prompt_state.prompt.positive.replace("\n", "").replace("\"", ""));
            let ws = ws.replace("NEGATIVE_INPUT", &prompt_state.prompt.negative.replace("\n", "").replace("\"", ""));
            let ws = ws.replace("11223344", &format!("{}", prompt_state.seed));
            // lets store that we queued this image
            request.set_metadata_id(machine.id);
            request.set_body(ws.as_bytes().to_vec());
            Self::update_progress(cx, &self.ui, machine.id, true, 0, 1);
            cx.http_request(live_id!(prompt), request);
            machine.running = Some(RunningPrompt {
                steps_counter: 0,
                prompt_state: prompt_state.clone(),
                _started: Instant::now(),
            });
            return
        }
        self.queue.push(prompt_state);
    }
    
    fn fetch_image(&self, cx: &mut Cx, machine_id: LiveId, image_name: &str) {
        let machine = self.machines.iter().find( | v | v.id == machine_id).unwrap();
        let url = format!("http://{}/view?filename={}&subfolder=&type=output", machine.ip, image_name);
        let mut request = HttpRequest::new(url, HttpMethod::GET);
        request.set_metadata_id(machine.id);
        cx.http_request(live_id!(image), request);
    }
    
    fn open_web_socket(&self, cx: &mut Cx) {
        for machine in &self.machines {
            let url = format!("ws://{}/ws?clientId={}", machine.ip, "1234");
            let request = HttpRequest::new(url, HttpMethod::GET);
            cx.web_socket_open(machine.id, request);
        }
    }
    
    fn update_progress(cx: &mut Cx, ui: &WidgetRef, machine: LiveId, active: bool, steps: usize, total: usize) {
        let progress_id = match machine {
            live_id!(m1) => id!(progress1),
            live_id!(m2) => id!(progress2),
            live_id!(m3) => id!(progress3),
            live_id!(m4) => id!(progress4),
            live_id!(m5) => id!(progress5),
            live_id!(m6) => id!(progress6),
            _ => panic!()
        };
        ui.get_frame(progress_id).apply_over(cx, live!{
            draw_bg: {active: (if active {1.0}else {0.0}), progress: (steps as f64 / total as f64)}
        });
        ui.redraw(cx);
    }
    
    fn set_current_image_by_item_id_and_row(&mut self, cx: &mut Cx, item_id: u64, row: usize) {
        self.ui.redraw(cx);
        if let Some(ImageListItem::ImageRow {prompt_hash: _, image_count, image_files}) = self.filtered.list.get(item_id as usize) {
            self.current_image = Some(image_files[row.min(*image_count)].clone());
        }
    }
    
    fn load_inputs_from_prompt_hash(&mut self, cx: &mut Cx, prompt_hash: LiveId) {
        if let Some(prompt_file) = self.db.prompt_files.iter().find( | v | v.prompt_hash == prompt_hash) {
            self.ui.get_text_input(id!(positive)).set_text(&prompt_file.prompt.positive);
            self.ui.get_text_input(id!(negative)).set_text(&prompt_file.prompt.negative);
            self.ui.redraw(cx);
        }
    }
    
    fn select_next_image(&mut self, cx: &mut Cx) {
        self.ui.redraw(cx);
        if let Some(current_image) = &self.current_image {
            if let Some(pos) = self.filtered.flat.iter().position( | v | *v == *current_image) {
                if pos + 1 < self.filtered.flat.len() {
                    self.current_image = Some(self.filtered.flat[pos + 1].clone());
                }
            }
        }
    }
    
    
    fn select_prev_image(&mut self, cx: &mut Cx) {
        self.ui.redraw(cx);
        if let Some(current_image) = &self.current_image {
            if let Some(pos) = self.filtered.flat.iter().position( | v | *v == *current_image) {
                if pos > 0 {
                    self.current_image = Some(self.filtered.flat[pos - 1].clone());
                }
            }
        }
    }
    
    fn render(&mut self, cx: &mut Cx) {
        let positive = self.ui.get_text_input(id!(positive)).get_text();
        //let keyword_input = self.ui.get_text_input(id!(keyword_input)).get_text();
        let negative = self.ui.get_text_input(id!(negative)).get_text();
        let batch_size = self.ui.get_drop_down(id!(batch_mode_dropdown)).get_selected() + 1;
        let workflow_id = self.ui.get_drop_down(id!(workflow_dropdown)).get_selected();
        let workflow = self.workflows[workflow_id].name.clone();
        for _ in 0..batch_size {
            self.last_seed += 1;
            self.send_prompt(cx, PromptState {
                prompt: Prompt {
                    positive: positive.clone(),
                    negative: negative.clone(),
                },
                workflow: workflow.clone(),
                seed: self.last_seed as u64
            });
        }
    }
}

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        if self.db.handle_decoded_images(cx) {
            self.ui.redraw(cx);
        }
        
        let image_list = self.ui.get_list_view_set(ids!(image_list));
        
        if let Event::Draw(event) = event {
            let cx = &mut Cx2d::new(cx, event);
            if let Some(current_image) = &self.current_image {
                let tex = self.db.get_image_texture(current_image);
                self.ui.get_image(id!(image_view.image)).set_texture(tex.clone());
                self.ui.get_image(id!(big_image.image)).set_texture(tex);
            }
            
            while let Some(next) = self.ui.draw_widget(cx).hook_widget() {
                
                if let Some(mut image_list) = image_list.has_widget(&next).borrow_mut() {
                    // alright now we draw the items
                    image_list.set_item_range(0, self.filtered.list.len() as u64, 1);
                    while let Some(item_id) = image_list.next_visible_item(cx) {
                        if let Some(item) = self.filtered.list.get(item_id as usize) {
                            match item {
                                ImageListItem::Prompt {prompt_hash} => {
                                    let group = self.db.prompt_files.iter().find( | v | v.prompt_hash == *prompt_hash).unwrap();
                                    let item = image_list.get_item(cx, item_id, live_id!(PromptGroup)).unwrap();
                                    item.get_label(id!(prompt)).set_label(&group.prompt.positive);
                                    item.draw_widget_all(cx);
                                }
                                ImageListItem::ImageRow {prompt_hash: _, image_count, image_files} => {
                                    let item = image_list.get_item(cx, item_id, id!(Empty.ImageRow1.ImageRow2)[*image_count]).unwrap();
                                    let rows = item.get_frame_set(ids!(row1, row2, row3));
                                    for (index, row) in rows.iter().enumerate() {
                                        if index >= *image_count {break}
                                        // alright we need to query our png cache for an image.
                                        let tex = self.db.get_image_texture(&image_files[index]);
                                        row.get_image(id!(img)).set_texture(tex);
                                    }
                                    item.draw_widget_all(cx);
                                }
                            }
                        }
                    }
                }
            }
            return
        }
        
        for event in event.network_responses() {
            match &event.response {
                NetworkResponse::WebSocketString(s) => {
                    if s.contains("execution_error") { // i dont care to expand the json def for this one
                        log!("Got execution error for {} {}", event.request_id, s);
                    }
                    else {
                        match ComfyUIMessage::deserialize_json(&s) {
                            Ok(data) => {
                                if data._type == "status" {
                                    if let Some(status) = data.data.status {
                                        if status.exec_info.queue_remaining == 0 {
                                            if let Some(machine) = self.machines.iter_mut().find( | v | {v.id == event.request_id}) {
                                                machine.running = None;
                                                Self::update_progress(cx, &self.ui, event.request_id, false, 0, 1);
                                            }
                                            if let Some(prompt) = self.queue.pop() {
                                                log!("QUEUED PROMPT!");
                                                self.send_prompt(cx, prompt);
                                            }
                                        }
                                    }
                                }
                                else if data._type == "executed" {
                                    if let Some(output) = &data.data.output {
                                        if let Some(image) = output.images.first() {
                                            if let Some(machine) = self.machines.iter_mut().find( | v | {v.id == event.request_id}) {
                                                if let Some(running) = machine.running.take() {
                                                    log!("Number of steps: {}", running.steps_counter);
                                                    machine.fetching = Some(running);
                                                    self.fetch_image(cx, event.request_id, &image.filename);
                                                }
                                            }
                                        }
                                    }
                                }
                                else if data._type == "progress" {
                                    // draw the progress bar / progress somewhere
                                    if let Some(machine) = self.machines.iter_mut().find( | v | {v.id == event.request_id}) {
                                        if let Some(running) = &mut machine.running {
                                            running.steps_counter += 1;
                                            let total = self.workflows.iter().find( | v | v.name == running.prompt_state.workflow).unwrap().total_steps;
                                            Self::update_progress(cx, &self.ui, event.request_id, true, running.steps_counter, total);
                                        }
                                    }
                                    //self.set_progress(cx, &format!("Step {}/{}", data.data.value.unwrap_or(0), data.data.max.unwrap_or(0)))
                                }
                            }
                            Err(err) => {
                                log!("Error parsing JSON {:?} {:?}", err, s);
                            }
                        }
                    }
                }
                NetworkResponse::WebSocketBinary(bin) => {
                    log!("Got Binary {}", bin.len());
                }
                NetworkResponse::HttpResponse(res) => {
                    // alright we got an image back
                    match event.request_id {
                        live_id!(prompt) => if let Some(_data) = res.get_string_body() { // lets check if the prompt executed
                        }
                        live_id!(image) => if let Some(data) = res.get_body() {
                            if let Some(machine) = self.machines.iter_mut().find( | v | {v.id == res.metadata_id}) {
                                if let Some(fetching) = machine.fetching.take() {
                                    
                                    // lets write our image to disk properly
                                    self.current_image = Some(self.db.add_png_and_prompt(fetching.prompt_state, data));
                                    self.filtered.filter_db(&self.db, "", false);
                                    // alright we got a png. lets decode it and stuff it in our image viewer
                                    //let big_list = self.ui.get_list_view(id!(big_list));
                                    //let image_id = self.num_images;
                                    //self.num_images += 1;
                                    //let item = big_list.get_item(cx, image_id, live_id!(Image)).unwrap().as_image();
                                    //item.load_png_from_data(cx, data);
                                    
                                    self.ui.redraw(cx);
                                }
                            }
                        }
                        _ => panic!()
                    }
                }
                e => {
                    log!("{} {:?}", event.request_id, e)
                }
            }
        }
        
        let actions = self.ui.handle_widget_event(cx, event);
        
        if let Event::KeyDown(KeyEvent {is_repeat: false, key_code: KeyCode::ReturnKey, ..}) = event {
            self.render(cx);
        }
        
        if self.ui.get_button(id!(render)).clicked(&actions) {
            self.render(cx);
        }
        
        if let Some(change) = self.ui.get_text_input(id!(search)).changed(&actions) {
            self.filtered.filter_db(&self.db, &change, false);
            self.ui.redraw(cx);
            image_list.set_first_id(0);
        }
        
        if let Some(e) = self.ui.get_frame(id!(image_view)).finger_down(&actions) {
            if e.tap_count >1 {
                self.ui.get_frame(id!(big_image)).set_visible(true);
                self.ui.redraw(cx);
            }
        }
        
        if let Some(e) = self.ui.get_frame(id!(big_image)).finger_down(&actions) {
            if e.tap_count >1 {
                self.ui.get_frame(id!(big_image)).set_visible(false);
                self.ui.redraw(cx);
            }
        }
        
        if let Some(ke) = self.ui.get_frame_set(ids!(image_view, big_image)).key_down(&actions) {
            match ke.key_code {
                KeyCode::ArrowDown => {
                    self.select_next_image(cx);
                }
                KeyCode::ArrowUp => {
                    self.select_prev_image(cx);
                }
                _ => ()
            }
        }
        
        for (item_id, item) in image_list.items_with_actions(&actions) {
            // check for actions inside the list item
            let rows = item.get_frame_set(ids!(row1, row2));
            for (index, row) in rows.iter().enumerate() {
                if let Some(fd) = row.finger_down(&actions) {
                    self.set_current_image_by_item_id_and_row(cx, item_id, index);
                    if fd.tap_count == 2 {
                        if let ImageListItem::ImageRow {prompt_hash, ..} = self.filtered.list[item_id as usize] {
                            self.load_inputs_from_prompt_hash(cx, prompt_hash);
                        }
                    }
                }
            }
            if let Some(fd) = item.as_frame().finger_down(&actions) {
                if fd.tap_count == 2 {
                    if let ImageListItem::Prompt {prompt_hash} = self.filtered.list[item_id as usize] {
                        self.load_inputs_from_prompt_hash(cx, prompt_hash);
                    }
                }
            }
        }
    }
}