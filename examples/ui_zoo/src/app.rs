use makepad_widgets::*;
use makepad_platform::live_atomic::*;


live_design!{
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_widgets::vectorline::*;
    import makepad_draw::shader::std::*;
    import makepad_example_ui_zoo::demofiletree::*;

    COLOR_BG = #4

    COLOR_D_0 = #00000000
    COLOR_D_1 = #00000011
    COLOR_D_2 = #00000022
    COLOR_D_3 = #00000033
    COLOR_D_4 = #00000066
    COLOR_D_5 = #00000099
    COLOR_D = #000000FF

    COLOR_U_0 = #FFFFFF00
    COLOR_U_1 = #FFFFFF11
    COLOR_U_2 = #FFFFFF22
    COLOR_U_3 = #FFFFFF33
    COLOR_U_4 = #FFFFFF66
    COLOR_U_5 = #FFFFFF88
    COLOR_U = #FFFFFFFF

    COLOR_DIVIDER = (COLOR_D_1)
    COLOR_TEXT_P = (COLOR_U_4)
    COLOR_TEXT_HL = (COLOR_U_5)
    COLOR_CONTAINER = (COLOR_D_2)
    COLOR_HEADER = (COLOR_U_1)
    COLOR_ACCENT = #f40

    SPACE_FACTOR = 10.0 // Increase for a less dense layout
    SPACE_0 = 0.0
    SPACE_1 = (0.5 * (SPACE_FACTOR))
    SPACE_2 = (1 * (SPACE_FACTOR))
    SPACE_3 = (2 * (SPACE_FACTOR))

    MSPACE_0 = {top: (SPACE_0), right: (SPACE_0), bottom: (SPACE_0), left: (SPACE_0)}
    MSPACE_1 = {top: (SPACE_1), right: (SPACE_1), bottom: (SPACE_1), left: (SPACE_1)}
    MSPACE_H_1 = {top: (SPACE_0), right: (SPACE_1), bottom: (SPACE_0), left: (SPACE_1)}
    MSPACE_V_1 = {top: (SPACE_1), right: (SPACE_0), bottom: (SPACE_1), left: (SPACE_0)}
    MSPACE_2 = {top: (SPACE_2), right: (SPACE_2), bottom: (SPACE_2), left: (SPACE_2)}
    MSPACE_H_2 = {top: (SPACE_0), right: (SPACE_2), bottom: (SPACE_0), left: (SPACE_2)}
    MSPACE_V_2 = {top: (SPACE_2), right: (SPACE_0), bottom: (SPACE_2), left: (SPACE_0)}

    ZooHeader = <View> {
        width: Fill, height: Fit,
        flow: Down
        align: { x: 0.0, y: 0.5},
        show_bg: false,
        padding: 15., margin:{bottom:10.}
        spacing: 10.,
        divider = <View> {
            height: 2.
            width: Fill,
            show_bg: true
            draw_bg: {color: (COLOR_DIVIDER)}
        }
        title = <Label> {
            draw_text: {
                text_style: {
                    line_spacing:1.0,
                    font_size: 14
                }
            }
            text: "Header"
        }
    }

    ZooGroup = <RoundedView>{
        height: Fit, width: Fill,
        flow: Right,
        align: { x: 0.0, y: 0.5},
        padding: 10.,
        show_bg: true;
        draw_bg: { color: (COLOR_CONTAINER) }
    }

    ZooTitle = <View> {
        width: Fit, height: Fit,
        flow: Down,
        align: { x: 0.0, y: 0.5},
        margin: 20., padding: 0.,
        spacing: 10.,
        show_bg: false,
        title = <Label> {
            draw_text: {
                color: (COLOR_TEXT_HL)
                text_style: {
                    line_spacing:1.0
                    font_size: 25
                }
            }
            text: "Makepad UI Zoo"
        }
    }

    ZooDesc = <Label> {
        margin: {top: 10.}, padding: 0.,
        draw_text: {
            text_style: {
                line_spacing: 1.5
            }
        }
        text: ""
    }

    ZooBlock = <RoundedView> {
        width: 50., height: 50.
        margin: 0., padding: 0.,
        spacing: 0.,

        show_bg: true;
        draw_bg: {
            color: #ff0
            fn get_color(self) -> vec4 {
                //return #000
                return mix(self.color, self.color*0.7, self.pos.y);
            }
            radius: 5.
        }
    }

    App = {{App}} {
        ui: <Window>{
            width: Fill, height: Fill,
            show_bg: true,
            draw_bg: {
                fn pixel(self) -> vec4 {
                    return (COLOR_BG);
                }
            }

            caption_bar = {
                visible: true,
                margin: {left: -100},
                caption_label = { label = {text: "Makepad UI Zoo"} }
            },

            body = <View>{
                width: Fill, height: Fill,
                flow: Right
                margin: 0., padding: 0.
                spacing: 0.

                <View>{
                    width: Fill, height: Fill,
                    flow: Down,
                    spacing: 10.,
                    show_bg: false,
                    scroll_bars: <ScrollBars>{}

                    <ZooTitle>{}

                    <ZooHeader>{
                        title = {text: "View 1" }
                        <ZooDesc>{text:"This is a gray view with flow set to Right\nTo show the extend, the background has been enabled using show_bg and a gray pixelshader has been provided to draw_bg."}
                        <View> {
                            height: Fit
                            flow: Right,
                            show_bg: true,
                            draw_bg: { color: (COLOR_CONTAINER) }
                            padding: 10.
                            spacing: 10.
                            <ZooBlock>{draw_bg:{color: #f00}}
                            <ZooBlock>{draw_bg:{color: #ff0}}
                            <ZooBlock>{draw_bg:{color: #00f}}
                        }

                        <ZooDesc>{ text:"This is a view with flow set to Down" }
                        <View> {
                            height: Fit,
                            flow: Down,
                            show_bg: true,
                            draw_bg: { color: (COLOR_D_2) }
                            padding: 10.
                            spacing: 10.
                            <ZooBlock>{draw_bg:{color: #f00}}
                            <ZooBlock>{draw_bg:{color: #ff0}}
                            <ZooBlock>{draw_bg:{color: #00f}}
                        }

                        <ZooDesc>{text:"This view is bigger on the inside"}
                        <View>{
                            width: 150, height: 150,
                            flow: Right,
                            show_bg: true,
                            draw_bg: { color: (COLOR_CONTAINER) }
                            scroll_bars: <ScrollBars>{}

                            <View>{
                                width: Fit, height: Fit,
                                flow: Down,
                                show_bg: false,
                                padding: 0
                                spacing: 10
                                <ZooBlock>{draw_bg:{color: #f00}}
                                <ZooBlock>{draw_bg:{color: #ff0}}
                                <ZooBlock>{draw_bg:{color: #00f}}
                                <ZooBlock>{draw_bg:{color: #0f0}}
                            }

                            <View>{
                                width: Fit, height: Fit,
                                flow: Down,
                                show_bg: false,
                                padding: 0
                                spacing: 10
                                <ZooBlock>{draw_bg:{color: #f00}}
                                <ZooBlock>{draw_bg:{color: #ff0}}
                                <ZooBlock>{draw_bg:{color: #00f}}
                                <ZooBlock>{draw_bg:{color: #0f0}}
                            }

                            <View>{
                                width: Fit, height: Fit,
                                flow: Down,
                                show_bg: false,
                                padding: 0
                                spacing: 10
                                <ZooBlock>{draw_bg:{color: #f00}}
                                <ZooBlock>{draw_bg:{color: #ff0}}
                                <ZooBlock>{draw_bg:{color: #00f}}
                                <ZooBlock>{draw_bg:{color: #0f0}}
                            }

                            <View>{
                                width: Fit, height: Fit,
                                flow: Down,
                                show_bg: false,
                                padding: 0
                                spacing: 10
                                <ZooBlock>{draw_bg:{color: #f00}}
                                <ZooBlock>{draw_bg:{color: #ff0}}
                                <ZooBlock>{draw_bg:{color: #00f}}
                                <ZooBlock>{draw_bg:{color: #0f0}}
                            }

                            <View>{
                                width: Fit, height: Fit,
                                flow: Down,
                                show_bg: false,
                                padding: 0
                                spacing: 10
                                <ZooBlock>{draw_bg:{color: #f00}}
                                <ZooBlock>{draw_bg:{color: #ff0}}
                                <ZooBlock>{draw_bg:{color: #00f}}
                            }
                        }
                    }

                    <ZooHeader>{
                        title = {text:"RoundedView"}
                        <ZooDesc>{
                            text:"This is a Rounded View. Please note that the radius has to be represented as a float value (with a decimal point) to work. Also note that instead of replacing the main pixel shader - you now replace get_color instead so the main shader can take care of rendering the radius."
                        }
                        <RoundedView> {
                            height: Fit
                            flow: Right,
                            padding: 10
                            spacing: 10
                            show_bg: true,
                            draw_bg: {
                                color: (COLOR_CONTAINER),
                                radius: 10.
                            }
                            <ZooBlock>{draw_bg:{color: #f00}}
                            <ZooBlock>{draw_bg:{color: #ff0}}
                            <ZooBlock>{draw_bg:{color: #00f}}
                        }
                    }

                    <ZooHeader>{
                        title = {text:"Button"}
                        <ZooDesc>{text:"A small clickable region"}
                        basicbutton = <Button> { text: "I can be clicked" }

                        iconbutton = <Button> {
                            draw_icon: {
                                svg_file: dep("crate://self/resources/Icon_Favorite.svg"),
                                color: #000;
                                brightness: 0.8;
                            }
                            text:"I can have a lovely icon!"

                            icon_walk: {
                                width: 30, height: Fit,
                                margin: 14,
                            }
                        }

                        styledbutton = <Button> {
                            draw_bg: {
                                fn pixel(self) -> vec4 {
                                    return #f40 + self.pressed * vec4(1., 1., 1., 1.)
                                }
                            }
                            draw_text: {
                                fn get_color(self) -> vec4 {
                                    return #fff - vec4(0., 0.1, 0.4, 0.) * self.hover - self.pressed * vec4(1., 1., 1., 0.);
                                }
                            }
                            text: "I can be styled!"
                        }
                    }

                    <ZooHeader>{
                        title = {text:"TextInput"}
                        <ZooDesc>{text:"Simple 1 line textbox"}
                        <ZooGroup>{
                            simpletextinput= <TextInput> {
                                text: "This is inside a text input!"
                            }

                            simpletextinput_outputbox = <Label> {
                                text: "Output"
                            }
                        }
                    }

                    <ZooHeader>{
                        title = {text:"Label"}
                        <ZooDesc> { text:"Simple 1 line textbox" }
                        <ZooGroup>{ <Label> { text: "This is a small line of text" } }
                        <ZooGroup>{
                            <Label> {
                                draw_text: {
                                    color: (COLOR_ACCENT)
                                    text_style: {
                                        font_size: 20,
                                    }
                                },
                                text: "You can style text using colors and fonts"
                            }
                        }
                        <ZooGroup>{
                            <Label>{
                                draw_text: {
                                    fn get_color(self) ->vec4{
                                        return mix((COLOR_ACCENT), #FF00, self.pos.x)
                                    }
                                    color: #ffc
                                    text_style: {
                                        font_size: 40.,
                                    }
                                },
                                text: "OR EVEN SOME PIXELSHADERS"
                            }
                        }
                    }

                    <ZooHeader>{
                        title = { text:"Slider" }
                        <ZooDesc> { text:"A parameter dragger" }
                        <ZooGroup> {
                            <Slider> {
                                text: "Parameter"
                                align: { x: 0.0, y: 0.0}
                                height: 25.,
                                draw_text: {
                                    color: (COLOR_TEXT_P)
                                    height: Fit,
                                    text_style: {
                                        line_spacing: 1.5
                                        font_size: (THEME_FONT_SIZE_BASE)
                                    }
                                }
                                text_input: {
                                    cursor_margin_bottom: (SPACE_1),
                                    cursor_margin_top: (SPACE_1),
                                    select_pad_edges: (SPACE_1),
                                    cursor_size: (SPACE_1),
                                    empty_message: "0",
                                    numeric_only: true,
                                    draw_bg: {
                                        color: (COLOR_D_0)
                                    },
                                }
                            }
                        }
                    }

                    <ZooHeader>{
                        title = {text:"DropDown"}
                        <ZooDesc>{text:"DropDown control. This control currently needs to be databound which needs some plumbing. In this sample there is a binding context struct in the main app struct - which gets bound on app start - and updated during handle_actions."}
                        <ZooGroup>{
                            dropdown = <DropDown>{
                                // width: 200, height: 30,
                                // draw_text: {
                                //     fn get_color(self) -> vec4 {
                                //         return mix(
                                //             mix(
                                //                 mix(
                                //                     (#xFFF8),
                                //                     (#xFFF8),
                                //                     self.focus
                                //                 ),
                                //                 (#xFFFF),
                                //                 self.hover
                                //             ),
                                //             (#x000A),
                                //             self.pressed
                                //         )
                                //     }
                                // }
                                labels: ["ValueOne", "ValueTwo", "Thrice", "FourthValue", "OptionE", "Hexagons"],
                                values: [ValueOne, ValueTwo, Thrice, FourthValue, OptionE, Hexagons]
                            }
                        }
                    }

                    <ZooHeader>{
                        title = {text:"DemoFileTree"}
                        <ZooDesc>{text:"DemoFileTree?"}
                        <ZooGroup> { <DemoFileTree>{ file_tree:{ height: 400 } } }
                    }

                    <ZooHeader>{
                        title = { text:"FoldHeader" }
                        <ZooDesc> { text:"This widget allows you to have a header with a foldbutton (has to be named fold_button for the magic to work)" }
                        <ZooGroup> {
                            thefoldheader= <FoldHeader> {
                                header: <View>{
                                    // width: Fill, height: Fit,
                                    // flow: Right,
                                    // show_bg: true,
                                    // draw_bg: { color: (COLOR_HEADER) }
                                    // padding: 5.0,
                                    fold_button = <FoldButton>{} <Label>{text: "Fold me!"}
                                }
                                body: <View>{
                                    width: Fill, height: Fit
                                    // show_bg: false,
                                    // padding: 5.0,
                                    <Label>{ text:"This is the body that can be folded away" }
                                }
                            }
                        }
                    }

                    <ZooHeader>{
                        title = {text:"Html"}
                        <ZooDesc>{text:"HTML Widget"}
                        <ZooGroup> {
                            <Html>{
                                width:Fill, height:Fit,
                                // font_size: (THEME_FONT_SIZE_BASE),
                                // flow: RightWrap,
                                // padding: 5,
                                // line_spacing: 10,
                                body:"This is <b>bold text</b>&nbsp;and&nbsp;<i>italic text</i>.<br/> <Button>Button</Button><br/> <block_quote>Blockquote<br/> <block_quote>Nested blockquote</block_quote> </block_quote> <b><i>Bold italic</i></b>"
                            }
                        }
                    }

                    <ZooHeader>{
                        title = {text:"Markdown"}
                        <ZooDesc>{text:"Markdown"}
                        <ZooGroup> {
                            <Markdown>{
                                width:Fill, height: Fit,
                                // font_size: (THEME_FONT_SIZE_BASE),
                                // flow: RightWrap,
                                // padding: 5,
                                // line_spacing: 10,
                                body:"# Headline 1 \n ## Headline 2 \n ### Headline 3 \n #### Headline 4 \n This is standard text with a  \n\n line break and a short ~~strike through~~ demo.\n\n *Italic text* \n\n **Bold text** \n\n - Bullet\n - Another bullet\n\n - Third bullet\n\n 1. Numbered list Bullet\n 2. Another list entry\n\n 3. Third list entry\n\n `Monospaced text`\n\n> This is a quote."
                            }
                        }
                    }

                    // TODO: SHOW
                    // SEEMS NOT TO WORK WITHOUT DUMMY DATA
                    // <ZooHeader>{
                    //     title = {text:"Portal List"}
                    //     <ZooDesc>{text:"Portal List"}
                    //     <ZooGroup> { <PortalList> { width: Fill, height: 100.} }
                    // }

                    // TODO: SHOW
                    <ZooHeader>{
                        title = {text:"Image"}
                        <ZooDesc>{text:"A static inline image from a resource."}
                        <ZooGroup> { <Image> { source: dep("crate://self/resources/ducky.png" ) } }
                    }

                    <ZooHeader>{
                        title = {text:"Link Label"}
                        <ZooDesc>{text:"Link Label"}
                        <ZooGroup> { <LinkLabel> { text: "Click me!"} }
                    }

                    <ZooHeader>{
                        title = {text:"CheckBox"}
                        <ZooDesc>{text:"Checkbox?"}
                        <ZooGroup> {
                            height: Fit
                            flow: Down,
                            <View> {
                                height: Fit
                                flow: Right
                                align: { x: 0.0, y: 0.5}
                                simplecheckbox = <CheckBox>{text:"Check me out!"}
                                simplecheckbox_output = <Label>{ text:"hmm" }
                                <CheckBox>{text:"Check me out!"}
                                <CheckBox>{text:"Check me out!"}
                            }
                            <View> {
                                height: Fit
                                flow: Right
                                align: { x: 0.0, y: 0.5}
                                <CheckBox>{text:"Check me out!"}
                                <CheckBox>{text:"Check me out!"}
                                <CheckBox>{text:"Check me out!"}
                            }
                            <View> {
                                height: Fit
                                flow: Right
                                align: { x: 0.0, y: 0.5}
                                <CheckBox>{text:"Check me out!"}
                                <CheckBox>{text:"Check me out!"}
                                <CheckBox>{text:"Check me out!"}
                            }
                        }
                    }

                    <ZooHeader>{
                        title = {text:"RadioButton"}
                        <ZooDesc>{text:"RadioButton?"}
                        <ZooGroup> {
                            flow: Down,
                            <View> {
                                height: Fit
                                flow: Right
                                align: { x: 0.0, y: 0.5 }
                                radios_demo = <View>{
                                    width: Fit, height: Fit,
                                    radio1 = <RadioButton>{
                                        label: "Option 1: yey"
                                    }
                                    radio2 = <RadioButton>{
                                        label: "Option 2: hah"
                                    }
                                    radio3 = <RadioButton>{
                                        label: "Option 3: hmm"
                                    }
                                    radio4 = <RadioButton>{
                                        label: "Option 4: all of the above"
                                    }
                                }
                            }
                            <View> {
                                height: Fit
                                flow: Right
                                align: { x: 0.0, y: 0.5 }
                                <View>{
                                    width: Fit, height: Fit,
                                    <RadioButton>{
                                        label: "Option 1: yey"
                                    }
                                    <RadioButton>{
                                        label: "Option 2: hah"
                                    }
                                    <RadioButton>{
                                        label: "Option 3: hmm"
                                    }
                                    <RadioButton>{
                                        label: "Option 4: all of the above"
                                    }
                                }
                            }
                            <View> {
                                height: Fit
                                flow: Right
                                align: { x: 0.0, y: 0.5 }
                                <View>{
                                    width: Fit, height: Fit,
                                    <RadioButton>{
                                        label: "Option 1: yey"
                                    }
                                    <RadioButton>{
                                        label: "Option 2: hah"
                                    }
                                    <RadioButton>{
                                        label: "Option 3: hmm"
                                    }
                                    <RadioButton>{
                                        label: "Option 4: all of the above"
                                    }
                                }
                            }
                        }
                    }

                    <ZooHeader>{
                        title = {text:"Slides View"}
                        width: Fill, height: Fit,
                        <ZooDesc>{text:"Slides View"}
                        <ZooGroup> {
                            <SlidesView> {
                                width: Fill, height: 400,

                                <SlideChapter> {
                                    title = {text: "Hey!"},
                                    <SlideBody> {text: "This is the 1st slide. Use your right\ncursor key to show the next slide."}
                                }
                                <Slide> {
                                    title = {text: "Slide #2"},
                                    <SlideBody> {text: "This is the 2nd slide. Use your left\ncursor key to show the previous slide."}
                                }
                            }
                        }
                    }

                    // TODO: SHOW
                    <ZooHeader>{
                        title = {text:"Dock"}
                        <ZooDesc>{text:"Dock"}
                        <ZooGroup> {
                            <Dock> {
                                height: 500., width: Fill

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
                                    tabs: [tab_c, tab_d],
                                    selected: 1
                                }

                                tab_a = Tab {
                                    name: "Tab A"
                                    kind: Container_A
                                }   

                                tab_b = Tab {
                                    name: "Tab B"
                                    kind: Container_B
                                }

                                tab_c = Tab {
                                    name: "Tab C"
                                    kind: Container_C
                                }   

                                tab_d = Tab {
                                    name: "Tab D"
                                    kind: Container_D
                                }

                                tab_e = Tab {
                                    name: "Tab E"
                                    kind: Container_E
                                }   

                                tab_f = Tab {
                                    name: "Tab F"
                                    kind: Container_F
                                }

                                Container_A = <RectView> {
                                    height: Fill, width: Fill
                                    draw_bg: { color: (COLOR_D_3) }
                                    padding: 10.,
                                    <Label> {text: "Hallo"}
                                }

                                Container_B = <RectView> {
                                    height: Fill, width: Fill
                                    draw_bg: { color: (COLOR_D_3) }
                                    padding: 10.,
                                    <Label> {text: "Kuckuck"}
                                }

                                Container_C = <RectView> {
                                    height: Fill, width: Fill
                                    draw_bg: { color: (COLOR_D_3) }
                                    padding: 10.,
                                    <Label> {text: "Ahoy"}
                                }

                                Container_D = <RectView> {
                                    height: Fill, width: Fill
                                    draw_bg: { color: (COLOR_D_3) }
                                    padding: 10.,
                                    <Label> {text: "Hi"}
                                }

                                Container_E = <RectView> {
                                    height: Fill, width: Fill
                                    draw_bg: { color: (COLOR_D_3) }
                                    padding: 10.,
                                    <Label> {text: "Ahoy"}
                                }

                                Container_F = <RectView> {
                                    height: Fill, width: Fill
                                    draw_bg: { color: (COLOR_D_3) }
                                    padding: 10.,
                                    <Label> {text: "Hi"}
                                }

                            }
                        }
                    }

                    // TODO: SHOW
                    // <ZooHeader>{
                    //     title = {text:"Stack Navigation"}
                    //     <ZooDesc>{text:"Stack Navigation"}
                    //     <ZooGroup> { <StackNavigation> { width: Fill, height: 100.} }
                    // }

                    ///////////////////////////////////////////////////////////////////////////////////////////////////////////////////
                    ///////////////////////////////////////////////////////////////////////////////////////////////////////////////////
                    ///////////////////////////////////////////////////////////////////////////////////////////////////////////////////

                    // TODO: SHOW
                    // SEEMS NOT TO WORK WITHOUT DUMMY DATA
                    // <ZooHeader>{
                    //     title = {text:"Portal List"}
                    //     <ZooDesc>{text:"Portal List"}
                    //     <ZooGroup> { <PortalList> { width: Fill, height: 100.} }
                    // }

                    // TODO: SHOW
                    // <ZooHeader>{
                    //     title = {text:"Flat list"}
                    //     <ZooDesc>{text:"Flat list"}
                    //     <ZooGroup> { <FlatList> { width: Fill, height: 100.} }
                    // }

                    // TODO: SHOW
                    // REFERENCE: https://github.com/project-robius/makepad_wonderous/blob/main/src/timeline/timeline_screen.rs#L242-L264
                    // <ZooHeader>{
                    //     title = {text: "Expandable Panel"}
                    //     <ZooDesc>{text: "Expandable Panel"}
                    //     <ZooGroup> {
                    //         expandable_panel = <ExpandablePanel> {
                    //             body = {
                    //                 flow: Down,
                    //                 spacing: 10,
                    //                 header = <View> { height: 100., width: 100., show_bg: true, draw_bg: { color: #0f0} }
                    //                 <View> { height: 100., width: 100., show_bg: true, draw_bg: { color: #f00} }
                    //             }

                    //             panel = {
                    //                 draw_bg: { color: #00f }

                    //                 scroll_handler = {
                    //                     draw_bg: {
                    //                         color: #aaa
                    //                         radius: 2.
                    //                     }
                    //                 }

                    //                 <View> { height: 100., width: 100., show_bg: true, draw_bg: { color: #ff0} }
                    //             }
                    //         }
                    //     }
                    // }

                    //  TODO: Slidepanel appears to be buggy
                    // <ZooHeader>{
                    //     title = {text:"SlidePanel"}
                    //     <ZooDesc>{text:"Slide panel?"}
                    //     <ZooGroup> {
                    //         <SlidePanel> {
                    //             width: (1000 * 0.175), height: (1000 * 0.175),
                    //             margin: 0.
                    //             side: Right,
                    //             <ZooHeader>{
                    //                 title = {text:"Image"}
                    //                 <ZooDesc>{text:"A static inline image from a resource."}
                    //                 <ZooGroup> {
                    //                     <Image> {
                    //                         width: (1000 * 0.175), height: (1000 * 0.175),
                    //                         margin: 0
                    //                         source: dep("crate://self/resources/ducky.png" ),
                    //                     }
                    //                 }
                    //             }
                    //         }
                    //     }
                    // }

                }
            }
        }
    }
}

app_main!(App);

#[derive(Live, LiveHook, PartialEq, LiveAtomic, Debug, LiveRead)]
pub enum DropDownEnum {
    #[pick]
    ValueOne,
    ValueTwo,
    Thrice,
    FourthValue,
    OptionE,
    Hexagons,
}

#[derive(Live, LiveHook, LiveRead, LiveRegister)]
pub struct DataBindingsForApp {
    #[live] fnumber: f32,
    #[live] inumber: i32,
    #[live] dropdown: DropDownEnum,
}
#[derive(Live, LiveHook)]
pub struct App {
    #[live] ui: WidgetRef,
    #[rust] counter: usize,
    #[rust(DataBindingsForApp::new(cx))] bindings: DataBindingsForApp
 }

impl LiveRegister for App {
    fn live_register(cx: &mut Cx) {
        crate::makepad_widgets::live_design(cx);
        crate::demofiletree::live_design(cx);
    }
}


impl MatchEvent for App{
    fn handle_actions(&mut self, cx: &mut Cx, actions:&Actions){
        let ui = self.ui.clone();

        ui.radio_button_set(ids!(radios_demo.radio1, radios_demo.radio2, radios_demo.radio3, radios_demo.radio4))
            .selected_to_visible(cx, &ui, actions, ids!(radios_demo.radio1, radios_demo.radio2, radios_demo.radio3, radios_demo.radio4));

        if let Some(txt) = self.ui.text_input(id!(simpletextinput)).changed(&actions){
            log!("TEXTBOX CHANGED {}", self.counter);
            self.counter += 1;
            let lbl = self.ui.label(id!(simpletextinput_outputbox));
            lbl.set_text_and_redraw(cx,&format!("{} {}" , self.counter, txt));
        }

        if self.ui.button(id!(basicbutton)).clicked(&actions) {
            log!("BASIC BUTTON CLICKED {}", self.counter);
            self.counter += 1;
            let btn = self.ui.button(id!(basicbutton));
            btn.set_text_and_redraw(cx,&format!("Clicky clicky! {}", self.counter));
        }

        if self.ui.button(id!(styledbutton)).clicked(&actions) {
            log!("STYLED BUTTON CLICKED {}", self.counter);
            self.counter += 1;
            let btn = self.ui.button(id!(styledbutton));
            btn.set_text_and_redraw(cx,&format!("Styled button clicked: {}", self.counter));
        }

        if self.ui.button(id!(iconbutton)).clicked(&actions) {
            log!("ICON BUTTON CLICKED {}", self.counter);
            self.counter += 1;
            let btn = self.ui.button(id!(iconbutton));
            btn.set_text_and_redraw(cx,&format!("Icon button clicked: {}", self.counter));
        }


        if let Some(check) = self.ui.check_box(id!(simplecheckbox)).changed(actions) {
            log!("CHECK BUTTON CLICKED {} {}", self.counter, check);
            self.counter += 1;
            let lbl = self.ui.label(id!(simplecheckbox_output));
            lbl.set_text_and_redraw(cx,&format!("{} {}" , self.counter, check));
        }

        if self.ui.fold_button(id!(folderbutton)).opening(actions) {
            log!("FOLDER BUTTON CLICKED {} {}", self.counter, 12);
//            self.ui.fold_header(id!(thefoldheader)).opened = true;

            self.counter += 1;
        }

        if self.ui.fold_button(id!(folderbutton)).closing(actions) {
            log!("FOLDER BUTTON CLICKED {} {}", self.counter, 12);



            self.counter += 1;
        }


        let mut db = DataBindingStore::new();
        db.data_bind(cx, actions, &self.ui, Self::data_bind);
        self.bindings.apply_over(cx, &db.nodes);

    }

    fn handle_startup(&mut self, cx: &mut Cx) {

        let ui = self.ui.clone();
        let db = DataBindingStore::from_nodes(self.bindings.live_read());
        Self::data_bind(db.data_to_widgets(cx, &ui));
    }
}

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}

impl App{
    pub fn data_bind(mut db: DataBindingMap) {
        db.bind(id!(dropdown), ids!(dropdown));
    }
}