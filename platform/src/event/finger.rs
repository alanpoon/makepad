#![allow(unused)]
#![allow(dead_code)]
use {
    std::cell::{Cell},
    crate::{
        makepad_micro_serde::*,
        makepad_live_tokenizer::{LiveErrorOrigin, live_error_origin},
        makepad_live_compiler::{
            LivePropType,
            LiveType,
            LiveTypeField,
            LiveFieldKind,
            LiveNode,
            LiveId,
            LiveModuleId,
            LiveTypeInfo,
            LiveNodeSliceApi
        },
        live_traits::{LiveNew, LiveHook, LiveRegister, LiveHookDeref, LiveApplyValue, LiveApply,LiveApplyReset, Apply},
        makepad_derive_live::*,
        makepad_math::*,
        makepad_live_id::{FromLiveId, live_id, live_id_num},
        event::{
            event::{Event, Hit}
        },
        window::WindowId,
        cx::Cx,
        area::Area,
    },
};

// Mouse events


#[derive(Clone, Copy, Debug, Default, SerBin, DeBin, SerJson, DeJson, PartialEq)]
pub struct KeyModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub logo: bool
}

impl KeyModifiers{
    /// Returns true if the primary key modifier is active (pressed).
    ///
    /// The primary modifier is Logo key (Command ⌘) on macOS
    /// and the Control key on all other platforms.
    pub fn is_primary(&self) -> bool {
        #[cfg(target_vendor = "apple")] {
            self.logo
        }
        #[cfg(not(target_vendor = "apple"))] {
            self.control
        }
    }

    fn any(&self)->bool{
        self.shift || self.control || self.alt || self.logo
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MouseDownEvent {
    pub abs: DVec2,
    pub button: usize,
    pub window_id: WindowId,
    pub modifiers: KeyModifiers,
    pub handled: Cell<Area>,
    pub time: f64
}


#[derive(Clone, Debug)]
pub struct MouseMoveEvent {
    pub abs: DVec2,
    pub window_id: WindowId,
    pub modifiers: KeyModifiers,
    pub time: f64,
    pub handled: Cell<Area>,
}

#[derive(Clone, Debug)]
pub struct MouseUpEvent {
    pub abs: DVec2,
    pub button: usize,
    pub window_id: WindowId,
    pub modifiers: KeyModifiers,
    pub time: f64
}

#[derive(Clone, Debug)]
pub struct MouseLeaveEvent {
    pub abs: DVec2,
    pub window_id: WindowId,
    pub modifiers: KeyModifiers,
    pub time: f64,
    pub handled: Cell<Area>,
}

#[derive(Clone, Debug)]
pub struct ScrollEvent {
    pub window_id: WindowId,
    pub scroll: DVec2,
    pub abs: DVec2,
    pub modifiers: KeyModifiers,
    pub handled_x: Cell<bool>,
    pub handled_y: Cell<bool>,
    pub is_mouse: bool,
    pub time: f64
}


// Touch events

#[derive(Clone, Copy, Debug)]
pub enum TouchState {
    Start,
    Stop,
    Move,
    Stable
}

#[derive(Clone, Debug)]
pub struct TouchPoint {
    pub state: TouchState,
    pub abs: DVec2,
    pub time: f64,
    pub uid: u64,
    pub rotation_angle: f64,
    pub force: f64,
    pub radius: DVec2,
    pub handled: Cell<Area>,
    pub sweep_lock: Cell<Area>,
}

#[derive(Clone, Debug)]
pub struct TouchUpdateEvent {
    pub time: f64,
    pub window_id: WindowId,
    pub modifiers: KeyModifiers,
    pub touches: Vec<TouchPoint>,
}


// Finger API


#[derive(Clone, Copy, Default, Debug, Live)]
#[live_ignore]
pub struct Margin {
    #[live] pub left: f64,
    #[live] pub top: f64,
    #[live] pub right: f64,
    #[live] pub bottom: f64
}
impl LiveRegister for Margin{}

impl LiveHook for Margin {
    fn skip_apply(&mut self, _cx: &mut Cx, _apply: &mut Apply, index: usize, nodes: &[LiveNode]) -> Option<usize> {
        if let Some(v) = nodes[index].value.as_float() {
            *self = Self {left: v, top: v, right: v, bottom: v};
            Some(index + 1)
        }
        else {
            None
        }
    }
}

impl Margin {
    pub fn left_top(&self) -> DVec2 {
        dvec2(self.left, self.top)
    }
    pub fn right_bottom(&self) -> DVec2 {
        dvec2(self.right, self.bottom)
    }
    pub fn size(&self) -> DVec2 {
        dvec2(self.left + self.right, self.top + self.bottom)
    }
    pub fn width(&self) -> f64 {
        self.left + self.right
    }
    pub fn height(&self) -> f64 {
        self.top + self.bottom
    }
    
    pub fn rect_contains_with_margin(pos: DVec2, rect: &Rect, margin: &Option<Margin>) -> bool {
        if let Some(margin) = margin {
            return
            pos.x >= rect.pos.x - margin.left
                && pos.x <= rect.pos.x + rect.size.x + margin.right
                && pos.y >= rect.pos.y - margin.top
                && pos.y <= rect.pos.y + rect.size.y + margin.bottom;
        }
        else {
            return rect.contains(pos);
        }
    }
}

pub const TAP_COUNT_TIME: f64 = 0.5;
pub const TAP_COUNT_DISTANCE: f64 = 10.0;

#[derive(Clone, Debug, Default, Eq, Hash, Copy, PartialEq, FromLiveId)]
pub struct DigitId(pub LiveId);

#[derive(Default, Clone)]
pub struct CxDigitCapture {
    digit_id: DigitId,
    pub area: Area,
    pub sweep_area: Area,
    pub switch_capture: Option<Area>,
    pub time: f64,
    pub abs_start: DVec2,
}

#[derive(Default, Clone)]
pub struct CxDigitTap {
    digit_id: DigitId,
    last_pos: DVec2,
    last_time: f64,
    count: u32
}

#[derive(Default, Clone)]
pub struct CxDigitHover {
    digit_id: DigitId,
    new_area: Area,
    area: Area,
}

#[derive(Default, Clone)]
pub struct CxFingers {
    pub first_mouse_button: Option<(usize, WindowId)>,
    captures: Vec<CxDigitCapture>,
    tap: CxDigitTap,
    hovers: Vec<CxDigitHover>,
    sweep_lock: Option<Area>,
}

impl CxFingers {
    /*
    pub (crate) fn get_captured_area(&self, digit_id: DigitId) -> Area {
        if let Some(cxdigit) = self.captures.iter().find( | v | v.digit_id == digit_id) {
            cxdigit.area
        }
        else {
            Area::Empty
        }
    }*/
    /*
    pub (crate) fn get_capture_time(&self, digit_id: DigitId) -> f64 {
        if let Some(cxdigit) = self.captures.iter().find( | v | v.digit_id == digit_id) {
            cxdigit.time
        }
        else {
            0.0
        }
    }*/
    
    pub (crate) fn find_digit_for_captured_area(&self, area: Area) -> Option<DigitId> {
        if let Some(digit) = self.captures.iter().find( | d | d.area == area) {
            return Some(digit.digit_id)
        }
        None
    }
    
    pub (crate) fn update_area(&mut self, old_area: Area, new_area: Area) {
        for hover in &mut self.hovers {
            if hover.area == old_area {
                hover.area = new_area;
            }
        }
        for capture in &mut self.captures {
            if capture.area == old_area {
                capture.area = new_area;
            }
            if capture.sweep_area == old_area {
                capture.sweep_area = new_area;
            }
        }
        if self.sweep_lock == Some(old_area) {
            self.sweep_lock = Some(new_area);
        }
    }
    
    pub (crate) fn new_hover_area(&mut self, digit_id: DigitId, new_area: Area) {
        for hover in &mut self.hovers {
            if hover.digit_id == digit_id {
                hover.new_area = new_area;
                return
            }
        }
        self.hovers.push(CxDigitHover {
            digit_id,
            area: Area::Empty,
            new_area,
        })
    }
    
    pub (crate) fn find_hover_area(&self, digit: DigitId) -> Area {
        for hover in &self.hovers {
            if hover.digit_id == digit {
                return hover.area
            }
        }
        Area::Empty
    }
    
    pub (crate) fn cycle_hover_area(&mut self, digit_id: DigitId) {
        if let Some(hover) = self.hovers.iter_mut().find( | v | v.digit_id == digit_id) {
            hover.area = hover.new_area;
            hover.new_area = Area::Empty;
        }
    }
    
    pub (crate) fn capture_digit(&mut self, digit_id: DigitId, area: Area, sweep_area: Area, time: f64, abs_start: DVec2) {
        /*if let Some(capture) = self.captures.iter_mut().find( | v | v.digit_id == digit_id) {
            capture.sweep_area = sweep_area;
            capture.area = area;
            capture.time = time;
            capture.abs_start = abs_start;
        }
        else {*/
        self.captures.push(CxDigitCapture {
            sweep_area,
            digit_id,
            area,
            time,
            abs_start,
            switch_capture: None
        })
        /*}*/
    }
    
    pub (crate) fn find_digit_capture(&mut self, digit_id: DigitId) -> Option<&mut CxDigitCapture> {
        self.captures.iter_mut().find( | v | v.digit_id == digit_id)
    }
    
    
    pub (crate) fn find_area_capture(&mut self, area: Area) -> Option<&mut CxDigitCapture> {
        self.captures.iter_mut().find( | v | v.area == area)
    }
    
    pub fn is_area_captured(&self, area: Area) -> bool {
        self.captures.iter().find( | v | v.area == area).is_some()
    }
    
    pub fn any_areas_captured(&self) -> bool {
        self.captures.len() > 0
    }
    
    pub (crate) fn release_digit(&mut self, digit_id: DigitId) {
        while let Some(index) = self.captures.iter_mut().position( | v | v.digit_id == digit_id) {
            self.captures.remove(index);
        }
    }
    
    pub (crate) fn remove_hover(&mut self, digit_id: DigitId) {
        while let Some(index) = self.hovers.iter_mut().position( | v | v.digit_id == digit_id) {
            self.hovers.remove(index);
        }
    }
    
    pub (crate) fn tap_count(&self) -> u32 {
        self.tap.count
    }
    
    pub (crate) fn process_tap_count(&mut self, pos: DVec2, time: f64) -> u32 {
        if (time - self.tap.last_time) < TAP_COUNT_TIME
            && pos.distance(&self.tap.last_pos) < TAP_COUNT_DISTANCE {
            self.tap.count += 1;
        }
        else {
            self.tap.count = 1;
        }
        self.tap.last_pos = pos;
        self.tap.last_time = time;
        return self.tap.count
    }
    
    pub (crate) fn process_touch_update_start(&mut self, time: f64, touches: &[TouchPoint]) {
        for touch in touches {
            if let TouchState::Start = touch.state {
                self.process_tap_count(touch.abs, time);
            }
        }
    }
    
    pub (crate) fn process_touch_update_end(&mut self, touches: &[TouchPoint]) {
        for touch in touches {
            let digit_id = live_id_num!(touch, touch.uid).into();
            match touch.state {
                TouchState::Stop => {
                    self.release_digit(digit_id);
                    self.remove_hover(digit_id);
                }
                TouchState::Start | TouchState::Move | TouchState::Stable => {
                    self.cycle_hover_area(digit_id);
                }
            }
        }
        self.switch_captures();
    }
    
    pub (crate) fn mouse_down(&mut self, button: usize, window_id:WindowId) {
        if self.first_mouse_button.is_none() {
            self.first_mouse_button = Some((button,window_id));
        }
    }
    
    pub (crate) fn switch_captures(&mut self) {
        for capture in &mut self.captures {
            if let Some(area) = capture.switch_capture {
                capture.area = area;
                capture.switch_capture = None;
            }
        }
    }
    
    pub (crate) fn mouse_up(&mut self, button: usize) {
        if self.first_mouse_button.is_some() && self.first_mouse_button.unwrap().0 == button {
            self.first_mouse_button = None;
            let digit_id = live_id!(mouse).into();
            self.release_digit(digit_id);
        }
    }
    
    pub (crate) fn test_sweep_lock(&mut self, sweep_area: Area) -> bool {
        if let Some(lock) = self.sweep_lock {
            if lock != sweep_area {
                return true
            }
        }
        false
    }
    
    pub fn sweep_lock(&mut self, area: Area) {
        if self.sweep_lock.is_none() {
            self.sweep_lock = Some(area);
        }
    }
    
    pub fn sweep_unlock(&mut self, area: Area) {
        if self.sweep_lock == Some(area) {
            self.sweep_lock = None;
        }
    }
    
}

#[derive(Clone, Copy, Debug)]
pub enum DigitDevice {
    Mouse {
        button: usize
    },
    Touch {
        uid: u64
    },
    XR {}
}

impl DigitDevice {
    pub fn is_touch(&self) -> bool {if let DigitDevice::Touch {..} = self {true}else {false}}
    pub fn is_mouse(&self) -> bool {if let DigitDevice::Mouse {..} = self {true}else {false}}
    pub fn is_xr(&self) -> bool {if let DigitDevice::XR {..} = self {true}else {false}}
    
    pub fn has_hovers(&self) -> bool {self.is_mouse() || self.is_xr()}
    
    pub fn mouse_button(&self) -> Option<usize> {if let DigitDevice::Mouse {button} = self {Some(*button)}else {None}}
    pub fn touch_uid(&self) -> Option<u64> {if let DigitDevice::Touch {uid} = self {Some(*uid)}else {None}}
    // pub fn xr_input(&self) -> Option<usize> {if let DigitDevice::XR(input) = self {Some(*input)}else {None}}
}


#[derive(Clone, Copy, Debug)]
pub struct FingerDownEvent {
    pub window_id: WindowId,
    pub abs: DVec2,
    
    pub digit_id: DigitId,
    pub device: DigitDevice,
    
    pub tap_count: u32,
    pub modifiers: KeyModifiers,
    pub time: f64,
    pub rect: Rect,
}

impl FingerDownEvent {
    pub fn mod_control(&self) -> bool {self.modifiers.control}
    pub fn mod_alt(&self) -> bool {self.modifiers.alt}
    pub fn mod_shift(&self) -> bool {self.modifiers.shift}
    pub fn mod_logo(&self) -> bool {self.modifiers.logo}
}

#[derive(Clone, Copy, Debug)]
pub struct FingerMoveEvent {
    pub window_id: WindowId,
    pub abs: DVec2,
    pub digit_id: DigitId,
    pub device: DigitDevice,
    
    pub tap_count: u32,
    pub modifiers: KeyModifiers,
    pub time: f64,
    
    pub abs_start: DVec2,
    pub rect: Rect,
    pub is_over: bool,
}

impl FingerMoveEvent {
    pub fn move_distance(&self) -> f64 {
        ((self.abs_start.x - self.abs.x).powf(2.) + (self.abs_start.y - self.abs.y).powf(2.)).sqrt()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FingerUpEvent {
    pub window_id: WindowId,
    pub abs: DVec2,
    pub capture_time: f64,
    
    pub digit_id: DigitId,
    pub device: DigitDevice,
    
    pub tap_count: u32,
    pub modifiers: KeyModifiers,
    pub time: f64,
    pub abs_start: DVec2,
    pub rect: Rect,
    pub is_over: bool,
    pub is_sweep: bool
}

impl FingerUpEvent {
    pub fn was_tap(&self) -> bool {
        self.time - self.capture_time < TAP_COUNT_TIME &&
        (self.abs_start - self.abs).length() < TAP_COUNT_DISTANCE
    }
    
    pub fn was_long_press(&self) -> bool {
        self.time - self.capture_time >= TAP_COUNT_TIME &&
        (self.abs_start - self.abs).length() < TAP_COUNT_DISTANCE
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HoverState {
    In,
    Over,
    Out
}

impl Default for HoverState {
    fn default() -> HoverState {
        HoverState::Over
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FingerHoverEvent {
    pub window_id: WindowId,
    pub abs: DVec2,
    pub digit_id: DigitId,
    pub device: DigitDevice,
    pub modifiers: KeyModifiers,
    pub time: f64,
    pub rect: Rect,
}

#[derive(Clone, Copy, Debug)]
pub struct FingerScrollEvent {
    pub window_id: WindowId,
    pub digit_id: DigitId,
    pub abs: DVec2,
    pub scroll: DVec2,
    pub device: DigitDevice,
    pub modifiers: KeyModifiers,
    pub time: f64,
    pub rect: Rect,
}

/*
pub enum HitTouch {
    Single,
    Multi
}*/


// Status


#[derive(Clone, Debug)]
pub struct HitOptions<H: FnMut(&Hit) -> bool = fn(&Hit) -> bool> {
    pub margin: Option<Margin>,
    pub sweep_area: Area,
    pub capture_overload: bool,
    /// A callback that can be used to mark a `Hit` as handled.
    /// If `None`, the default behavior is to return `true`, i.e.,
    /// to mark the `Hit` as handled.
    ///
    /// Marking a `Hit` as "handled" means that it will not be delivered
    /// to child widgets. This is useful when you want to prevent
    /// a `Hit` from being delivered to a child widget.
    ///
    /// If the callback returns `false`, the `Hit` is not marked as handled.
    /// This is useful when you want to allow a `Hit` to be handled in a given widget
    /// AND to be handled by that widgets' child widgets.
    pub mark_as_handled_fn: Option<H>,
}
impl Default for HitOptions {
    fn default() -> Self {
        Self {
            margin: None,
            sweep_area: Area::Empty,
            capture_overload: false,
            mark_as_handled_fn: None,
        }
    }
}

impl<H: FnMut(&Hit) -> bool> HitOptions<H> {
    pub fn with_sweep_area(self, area: Area) -> Self {
        Self {
            sweep_area: area,
            ..self
        }
    }
    pub fn with_margin(self, margin: Margin) -> Self {
        Self {
            margin: Some(margin),
            ..self
        }
    }
    pub fn with_capture_overload(self, capture_overload:bool) -> Self {
        Self {
            capture_overload,
            ..self
        }
    }

    /// See [`HitOptions::mark_as_handled_fn`].
    pub fn with_mark_as_handled_fn(self, mark_as_handled_fn: H) -> Self {
        Self {
            mark_as_handled_fn: Some(mark_as_handled_fn),
            ..self
        }
    }

    fn run_mark_as_handled_fn(&mut self, fei: &Hit) -> bool {
        if let Some(mark_as_handled_fn) = self.mark_as_handled_fn.as_mut() {
            mark_as_handled_fn(fei)
        } else {
            true
        }
    }
}


impl Event {
    pub fn hits(&self, cx: &mut Cx, area: Area) -> Hit {
        self.hits_with_options(cx, area, HitOptions::default())
    }

    pub fn hits_with_test<F>(&self, cx: &mut Cx, area: Area, hit_test: F) -> Hit
    where F: Fn(DVec2, &Rect, &Option<Margin>)->bool{
        self.hits_with_options_and_test(cx, area,  HitOptions::default(), hit_test)
    }

    pub fn hits_with_sweep_area(&self, cx: &mut Cx, area: Area, sweep_area: Area) -> Hit {
        self.hits_with_options(cx, area, HitOptions::default().with_sweep_area(sweep_area))
    }
    
    pub fn hits_with_capture_overload(&self, cx: &mut Cx, area: Area, capture_overload: bool) -> Hit {
        self.hits_with_options(cx, area, HitOptions::default().with_capture_overload(capture_overload))
    }

    /// See [`HitOptions::mark_as_handled_fn`].
    pub fn hits_with_mark_as_handled_fn<H>(&self, cx: &mut Cx, area: Area, mark_as_handled_fn: H) -> Hit
    where
        H: FnMut(&Hit) -> bool
    {
        self.hits_with_options(
            cx,
            area,
            HitOptions::<H> {
                margin: None,
                sweep_area: Area::Empty,
                capture_overload: false,
                mark_as_handled_fn: Some(mark_as_handled_fn),
            }
        )
    }

    /// See [`HitOptions::mark_as_handled_fn`] for mroe info about the `H` parameter.
    pub fn hits_with_options<H>(&self, cx: &mut Cx, area: Area, options: HitOptions<H>) -> Hit
    where
        H: FnMut(&Hit) -> bool,
    {
        self.hits_with_options_and_test(cx, area, options, |abs, rect, margin|{
            Margin::rect_contains_with_margin(abs, rect, margin)
        })
    }

    /// See [`HitOptions::mark_as_handled_fn`] for mroe info about the `H` parameter.
    pub fn hits_with_options_and_test<F, H>(&self, cx: &mut Cx, area: Area, mut options: HitOptions<H>, hit_test: F) -> Hit
    where
        F: Fn(DVec2, &Rect, &Option<Margin>) -> bool,
        H: FnMut(&Hit) -> bool,
    {
        if !area.is_valid(cx) {
            return Hit::Nothing
        }
        match self {
            Event::KeyFocus(kf) => {
                if area == kf.prev {
                    return Hit::KeyFocusLost(kf.clone())
                }
                else if area == kf.focus {
                    return Hit::KeyFocus(kf.clone())
                }
            },
            Event::KeyDown(kd) => {
                if cx.keyboard.has_key_focus(area) {
                    return Hit::KeyDown(kd.clone())
                }
            },
            Event::KeyUp(ku) => {
                if cx.keyboard.has_key_focus(area) {
                    return Hit::KeyUp(ku.clone())
                }
            },
            Event::TextInput(ti) => {
                if cx.keyboard.has_key_focus(area) {
                    return Hit::TextInput(ti.clone())
                }
            },
            Event::TextCopy(tc) => {
                if cx.keyboard.has_key_focus(area) {
                    return Hit::TextCopy(tc.clone());
                }
            },
            Event::TextCut(tc) => {
                if cx.keyboard.has_key_focus(area) {
                    return Hit::TextCut(tc.clone());
                }
            },
            Event::Scroll(e) => {
                let digit_id = live_id!(mouse).into();
                
                let rect = area.clipped_rect(&cx);
                if hit_test(e.abs, &rect, &options.margin) {
                    //fe.handled = true;
                    let device = DigitDevice::Mouse {
                        button: 0,
                    };
                    return Hit::FingerScroll(FingerScrollEvent {
                        abs: e.abs,
                        rect,
                        window_id: e.window_id,
                        digit_id,
                        device,
                        modifiers: e.modifiers,
                        time: e.time,
                        scroll: e.scroll
                    })
                }
            },
            Event::TouchUpdate(e) => {
                if cx.fingers.test_sweep_lock(options.sweep_area) {
                    return Hit::Nothing
                }
                for t in &e.touches {
                    let digit_id = live_id_num!(touch, t.uid).into();
                    let device = DigitDevice::Touch {
                        uid: t.uid,
                    };
                    
                    match t.state {
                        TouchState::Start => {
                            
                            if !options.capture_overload && !t.handled.get().is_empty() {
                                continue;
                            }
                            
                            if cx.fingers.find_area_capture(area).is_some(){
                                continue;
                            }
                            
                            let rect = area.clipped_rect(&cx);
                            if !hit_test(t.abs, &rect, &options.margin) {
                                continue;
                            }
                            
                            cx.fingers.capture_digit(digit_id, area, options.sweep_area, e.time, t.abs);

                            let fde = Hit::FingerDown(FingerDownEvent {
                                window_id: e.window_id,
                                abs: t.abs,
                                digit_id,
                                device,
                                tap_count: cx.fingers.tap_count(),
                                modifiers: e.modifiers,
                                time: e.time,
                                rect,
                            });
                            if options.run_mark_as_handled_fn(&fde) {
                                t.handled.set(area);
                            }
                            return fde;
                        }
                        TouchState::Stop => {
                            let tap_count = cx.fingers.tap_count();
                            let rect = area.clipped_rect(&cx);
                            if let Some(capture) = cx.fingers.find_area_capture(area) {
                                return Hit::FingerUp(FingerUpEvent {
                                    abs_start: capture.abs_start,
                                    rect,
                                    window_id: e.window_id,
                                    abs: t.abs,
                                    digit_id,
                                    device,
                                    tap_count,
                                    capture_time: capture.time,
                                    modifiers: e.modifiers,
                                    time: e.time,
                                    is_over: rect.contains(t.abs),
                                    is_sweep: false,
                                })
                            }
                        }
                        TouchState::Move => {
                            let tap_count = cx.fingers.tap_count();
                            //let hover_last = cx.fingers.get_hover_area(digit_id);
                            let rect = area.clipped_rect(&cx);
                            
                            //let handled_area = t.handled.get();
                            if !options.sweep_area.is_empty() {
                                if let Some(capture) = cx.fingers.find_digit_capture(digit_id) {
                                    if capture.switch_capture.is_none()
                                        && hit_test(t.abs, &rect, &options.margin) {
                                        if t.handled.get().is_empty() {
                                            if capture.area == area {
                                                let fme = Hit::FingerMove(FingerMoveEvent {
                                                    window_id: e.window_id,
                                                    abs: t.abs,
                                                    digit_id,
                                                    device,
                                                    tap_count,
                                                    modifiers: e.modifiers,
                                                    time: e.time,
                                                    abs_start: capture.abs_start,
                                                    rect,
                                                    is_over: true,
                                                });
                                                if options.run_mark_as_handled_fn(&fme) {
                                                    t.handled.set(area);
                                                }
                                                return fme;
                                            }
                                            else if capture.sweep_area == options.sweep_area { // take over the capture
                                                capture.switch_capture = Some(area);
                                                let fde = Hit::FingerDown(FingerDownEvent {
                                                    window_id: e.window_id,
                                                    abs: t.abs,
                                                    digit_id,
                                                    device,
                                                    tap_count: cx.fingers.tap_count(),
                                                    modifiers: e.modifiers,
                                                    time: e.time,
                                                    rect,
                                                });
                                                if options.run_mark_as_handled_fn(&fde) {
                                                    t.handled.set(area);
                                                }
                                                return fde;
                                            }
                                        }
                                    }
                                    else if capture.area == area { // we are not over the area
                                        if capture.switch_capture.is_none() {
                                            capture.switch_capture = Some(Area::Empty);
                                        }
                                        return Hit::FingerUp(FingerUpEvent {
                                            abs_start: capture.abs_start,
                                            rect,
                                            window_id: e.window_id,
                                            abs: t.abs,
                                            digit_id,
                                            device,
                                            tap_count,
                                            capture_time: capture.time,
                                            modifiers: e.modifiers,
                                            time: e.time,
                                            is_sweep: true,
                                            is_over: false,
                                        });
                                    }
                                }
                            }
                            else if let Some(capture) = cx.fingers.find_area_capture(area) {
                                return Hit::FingerMove(FingerMoveEvent {
                                    window_id: e.window_id,
                                    abs: t.abs,
                                    digit_id,
                                    device,
                                    tap_count,
                                    modifiers: e.modifiers,
                                    time: e.time,
                                    abs_start: capture.abs_start,
                                    rect,
                                    is_over: hit_test(t.abs, &rect, &options.margin),
                                })
                            }
                        }
                        TouchState::Stable => {}
                    }
                }
            }
            Event::MouseMove(e) => { // ok so we dont get hovers
                if cx.fingers.test_sweep_lock(options.sweep_area) {
                    return Hit::Nothing
                }
                
                let digit_id = live_id!(mouse).into();
                
                let tap_count = cx.fingers.tap_count();
                let hover_last = cx.fingers.find_hover_area(digit_id);
                let rect = area.clipped_rect(&cx);
                
                if let Some((button, _window_id)) = cx.fingers.first_mouse_button {
                    let device = DigitDevice::Mouse {
                        button,
                    };
                    //let handled_area = e.handled.get();
                    if !options.sweep_area.is_empty() {
                        if let Some(capture) = cx.fingers.find_digit_capture(digit_id) {
                            if capture.switch_capture.is_none()
                                && hit_test(e.abs, &rect, &options.margin) {
                                if e.handled.get().is_empty() {
                                    if capture.area == area {
                                        let fme = Hit::FingerMove(FingerMoveEvent {
                                            window_id: e.window_id,
                                            abs: e.abs,
                                            digit_id,
                                            device,
                                            tap_count,
                                            modifiers: e.modifiers,
                                            time: e.time,
                                            abs_start: capture.abs_start,
                                            rect,
                                            is_over: true,
                                        });
                                        if options.run_mark_as_handled_fn(&fme) {
                                            e.handled.set(area);
                                        }
                                        return fme;
                                    }
                                    else if capture.sweep_area == options.sweep_area { // take over the capture
                                        capture.switch_capture = Some(area);
                                        cx.fingers.new_hover_area(digit_id, area);
                                        let fde = Hit::FingerDown(FingerDownEvent {
                                            window_id: e.window_id,
                                            abs: e.abs,
                                            digit_id,
                                            device,
                                            tap_count: cx.fingers.tap_count(),
                                            modifiers: e.modifiers,
                                            time: e.time,
                                            rect,
                                        });
                                        if options.run_mark_as_handled_fn(&fde) {
                                            e.handled.set(area);
                                        }
                                        return fde;
                                    }
                                }
                            }
                            else if capture.area == area { // we are not over the area
                                if capture.switch_capture.is_none() {
                                    capture.switch_capture = Some(Area::Empty);
                                }
                                return Hit::FingerUp(FingerUpEvent {
                                    abs_start: capture.abs_start,
                                    rect,
                                    window_id: e.window_id,
                                    abs: e.abs,
                                    digit_id,
                                    device,
                                    tap_count,
                                    capture_time: capture.time,
                                    modifiers: e.modifiers,
                                    time: e.time,
                                    is_sweep: true,
                                    is_over: false,
                                });
                                
                            }
                        }
                    }
                    else if let Some(capture) = cx.fingers.find_area_capture(area) {
                        let event = Hit::FingerMove(FingerMoveEvent {
                            window_id: e.window_id,
                            abs: e.abs,
                            digit_id,
                            device,
                            tap_count,
                            modifiers: e.modifiers,
                            time: e.time,
                            abs_start: capture.abs_start,
                            rect,
                            is_over: hit_test(e.abs, &rect, &options.margin),
                        });
                        cx.fingers.new_hover_area(digit_id, area);
                        return event
                    }
                }
                else {
                    let device = DigitDevice::Mouse {
                        button: 0,
                    };
                    
                    let handled_area = e.handled.get();
                    
                    let fhe = FingerHoverEvent {
                        window_id: e.window_id,
                        abs: e.abs,
                        digit_id,
                        device,
                        modifiers: e.modifiers,
                        time: e.time,
                        rect,
                    };
                    
                    if hover_last == area {
                        if handled_area.is_empty() && hit_test(e.abs, &rect, &options.margin) {
                            let fhe = Hit::FingerHoverOver(fhe);
                            if options.run_mark_as_handled_fn(&fhe) {
                                e.handled.set(area);
                            }
                            cx.fingers.new_hover_area(digit_id, area);
                            return fhe;
                        }
                        else {
                            return Hit::FingerHoverOut(fhe);
                        }
                    }
                    else {
                        if handled_area.is_empty() && hit_test(e.abs, &rect, &options.margin) {
                            //let any_captured = cx.fingers.get_digit_for_captured_area(area);
                            cx.fingers.new_hover_area(digit_id, area);
                            let fhi = Hit::FingerHoverIn(fhe);
                            if options.run_mark_as_handled_fn(&fhi) {
                                e.handled.set(area);
                            }
                            return fhi;
                        }
                    }
                }
            },
            Event::MouseDown(e) => {
                if cx.fingers.test_sweep_lock(options.sweep_area) {
                    return Hit::Nothing
                }
                
                let digit_id = live_id!(mouse).into();
                
                if !options.capture_overload && !e.handled.get().is_empty() {
                    return Hit::Nothing
                }
                
                if cx.fingers.first_mouse_button.is_some() && cx.fingers.first_mouse_button.unwrap().0 != e.button{
                    return Hit::Nothing
                }
                
                let rect = area.clipped_rect(&cx);
                if !hit_test(e.abs, &rect, &options.margin) {
                    return Hit::Nothing
                }
                
                let device = DigitDevice::Mouse {
                    button: e.button,
                };
                
                if cx.fingers.find_digit_for_captured_area(area).is_some() {
                    return Hit::Nothing;
                }
                
                cx.fingers.capture_digit(digit_id, area, options.sweep_area, e.time, e.abs);
                cx.fingers.new_hover_area(digit_id, area);
                let fde = Hit::FingerDown(FingerDownEvent {
                    window_id: e.window_id,
                    abs: e.abs,
                    digit_id,
                    device,
                    tap_count: cx.fingers.tap_count(),
                    modifiers: e.modifiers,
                    time: e.time,
                    rect,
                });
                if options.run_mark_as_handled_fn(&fde) {
                    e.handled.set(area);
                }
                return fde;
            },
            Event::MouseUp(e) => {
                if cx.fingers.test_sweep_lock(options.sweep_area) {
                    return Hit::Nothing
                }
                
                if cx.fingers.first_mouse_button.is_some() && cx.fingers.first_mouse_button.unwrap().0 != e.button {
                    return Hit::Nothing
                }
                
                let digit_id = live_id!(mouse).into();
                
                let device = DigitDevice::Mouse {
                    button: e.button,
                };
                let tap_count = cx.fingers.tap_count();
                let rect = area.clipped_rect(&cx);
                
                if let Some(capture) = cx.fingers.find_area_capture(area) {
                    let is_over = hit_test(e.abs, &rect, &options.margin);
                    let event = Hit::FingerUp(FingerUpEvent {
                        abs_start: capture.abs_start,
                        rect,
                        window_id: e.window_id,
                        abs: e.abs,
                        digit_id,
                        device,
                        tap_count,
                        capture_time: capture.time,
                        modifiers: e.modifiers,
                        time: e.time,
                        is_over,
                        is_sweep: false,
                    });
                    if is_over {
                        cx.fingers.new_hover_area(digit_id, area);
                    }
                    return event
                }
            },
            Event::MouseLeave(e) => {
                if cx.fingers.test_sweep_lock(options.sweep_area) {
                    return Hit::Nothing;
                }
                let device = DigitDevice::Mouse { button: 0 };
                let digit_id = live_id!(mouse).into();
                let rect = area.clipped_rect(&cx);
                let hover_last = cx.fingers.find_hover_area(digit_id);
                let handled_area = e.handled.get();
                
                let fhe = FingerHoverEvent {
                    window_id: e.window_id,
                    abs: e.abs,
                    digit_id,
                    device,
                    modifiers: e.modifiers,
                    time: e.time,
                    rect,
                };
                if hover_last == area {
                    return Hit::FingerHoverOut(fhe);
                }
            },
            Event::DesignerPick(e) => {
               
                let rect = area.clipped_rect(&cx);
                if !hit_test(e.abs, &rect, &options.margin) {
                    return Hit::Nothing
                }
                // lets add our area to a handled vec?
                // but how will we communicate the widget?
                return Hit::DesignerPick(e.clone())
            },
            _ => ()
        };
        Hit::Nothing
    }
}
