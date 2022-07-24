use {
    std::ops::{ControlFlow, Try, FromResidual},
    std::any::TypeId,
    crate::makepad_platform::*,
    std::collections::BTreeMap,
};
pub use crate::frame_component;

pub trait FrameComponent: LiveApply {
    fn handle_component_event(
        &mut self,
        _cx: &mut Cx,
        _event: &mut Event,
        _fn_action: &mut dyn FnMut(&mut Cx, FramePath, Box<dyn FrameAction>)
    ) {}
    
    fn handle_event_iter(&mut self, cx: &mut Cx, event: &mut Event) -> Vec<FrameActionItem> {
        let mut actions = Vec::new();
        self.handle_component_event(cx, event, &mut | _, path, action | {
            actions.push(FrameActionItem {
                action: action,
                path
            });
        });
        actions
    }
    
    fn query_child(
        &mut self,
        _query: &QueryChild,
        _callback: &mut Option<&mut dyn FnMut(QueryInner)>
    ) -> QueryResult {
        return QueryResult::NotFound
    }
    
    fn draw_component(&mut self, cx: &mut Cx2d, walk: Walk, self_uid: FrameUid) -> DrawResult;
    fn get_walk(&self) -> Walk;
    
    // defaults
    fn redraw(&mut self, _cx: &mut Cx) {}
    
    fn draw_walk_component(&mut self, cx: &mut Cx2d, self_uid: FrameUid) -> DrawResult {
        self.draw_component(cx, self.get_walk(), self_uid)
    }
    
    fn create_child(
        &mut self,
        _cx: &mut Cx,
        _live_ptr: LivePtr,
        _at: CreateAt,
        _new_id: LiveId,
        _nodes: &[LiveNode]
    ) -> Option<&mut Box<dyn FrameComponent >> {
        None
    }
    
    fn template(
        &mut self,
        cx: &mut Cx,
        path: &[LiveId],
        new_id: LiveId,
        nodes: &[LiveNode]
    ) -> Option<&mut Box<dyn FrameComponent >> {
        // first we query the template
        if path.len() == 1{
            if let Some(live_ptr) = self.query_template(path[0]){
                return self.create_child(cx, live_ptr, CreateAt::Template, new_id, nodes)
            }
        }
        if let QueryResult::Found(QueryInner::Template(child, live_ptr)) =
        self.query_child(&QueryChild::Path(path), &mut None) {
            child.create_child(cx, live_ptr, CreateAt::Template, new_id, nodes)
        }
        else {
            None
        }
    }
    
    fn query_template(&self, _id: LiveId) -> Option<LivePtr> {
        None
    }
    
    fn type_id(&self) -> LiveType where Self: 'static {LiveType::of::<Self>()}
}

#[derive(Clone, Copy)]
pub enum CreateAt {
    Template,
    Begin,
    After(LiveId),
    Before(LiveId),
    End
}

pub enum QueryChild<'a> {
    Path(&'a [LiveId]),
    Uid(FrameUid),
}

pub enum QueryInner<'a> {
    Child(&'a mut Box<dyn FrameComponent >),
    Template(&'a mut Box<dyn FrameComponent >, LivePtr)
}

pub enum QueryResult<'a> {
    NotFound,
    Found(QueryInner<'a>)
}

impl<'a> QueryResult<'a> {
    pub fn child(value: &'a mut Box<dyn FrameComponent >) -> Self {
        Self::Found(QueryInner::Child(value))
    }
    pub fn template(value: &'a mut Box<dyn FrameComponent >, live_ptr: LivePtr) -> Self {
        Self::Found(QueryInner::Template(value, live_ptr))
    }
}

impl<'a> FromResidual for QueryResult<'a> {
    fn from_residual(residual: QueryInner<'a>) -> Self {
        Self::Found(residual)
    }
}

impl<'a> Try for QueryResult<'a> {
    type Output = ();
    type Residual = QueryInner<'a>;
    
    fn from_output(_: Self::Output) -> Self {
        QueryResult::NotFound
    }
    
    fn branch(self) -> ControlFlow<Self::Residual,
    Self::Output> {
        match self {
            Self::NotFound => ControlFlow::Continue(()),
            Self::Found(c) => ControlFlow::Break(c)
        }
    }
}

pub enum DrawResult {
    Done,
    UserDraw(FrameUid)
}

impl DrawResult {
    pub fn is_done(&self) -> bool {
        match self {
            Self::Done => true,
            _ => false
        }
    }
    pub fn not_done(&self) -> bool {
        match self {
            Self::Done => false,
            _ => true
        }
    }
}

impl FromResidual for DrawResult {
    fn from_residual(residual: FrameUid) -> Self {
        Self::UserDraw(residual)
    }
}

impl Try for DrawResult {
    type Output = ();
    type Residual = FrameUid;
    
    fn from_output(_: Self::Output) -> Self {
        DrawResult::Done
    }
    
    fn branch(self) -> ControlFlow<Self::Residual,
    Self::Output> {
        match self {
            Self::Done => ControlFlow::Continue(()),
            Self::UserDraw(c) => ControlFlow::Break(c)
        }
    }
}


generate_ref_cast_api!(FrameComponent);

pub trait FrameComponentFactory {
    fn new(&self, cx: &mut Cx) -> Box<dyn FrameComponent>;
}

#[derive(Default, LiveComponentRegistry)]
pub struct FrameComponentRegistry {
    pub map: BTreeMap<LiveType, (LiveComponentInfo, Box<dyn FrameComponentFactory>)>
}

pub trait FrameAction: 'static {
    fn type_id(&self) -> TypeId;
    fn box_clone(&self) -> Box<dyn FrameAction>;
}

impl<T: 'static + ? Sized + Clone> FrameAction for T {
    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    
    fn box_clone(&self) -> Box<dyn FrameAction> {
        Box::new((*self).clone())
    }
}

generate_clone_cast_api!(FrameAction);

impl Clone for Box<dyn FrameAction> {
    fn clone(&self) -> Box<dyn FrameAction> {
        self.as_ref().box_clone()
    }
}

pub struct FrameRef(Option<Box<dyn FrameComponent >>);

impl FrameRef {
    pub fn empty() -> Self {Self (None)}
    
    pub fn as_uid(&self) -> FrameUid {
        if let Some(inner) = &self.0 {
            FrameUid(&*inner as *const _ as u64)
        }
        else {
            FrameUid(0)
        }
    }
    
    pub fn as_ref(&self) -> Option<&Box<dyn FrameComponent >> {
        self.0.as_ref()
    }
    pub fn as_mut(&mut self) -> Option<&mut Box<dyn FrameComponent >> {
        self.0.as_mut()
    }
    
    pub fn handle_component_event(&mut self, cx: &mut Cx, event: &mut Event, dispatch_action: &mut dyn FnMut(&mut Cx, FramePath, Box<dyn FrameAction>)) {
        if let Some(inner) = &mut self.0 {
            return inner.handle_component_event(cx, event, dispatch_action)
        }
    }
    
    pub fn handle_event_iter(&mut self, cx: &mut Cx, event: &mut Event) -> Vec<FrameActionItem> {
        if let Some(inner) = &mut self.0 {
            return inner.handle_event_iter(cx, event)
        }
        Vec::new()
    }
    
    pub fn query_child(&mut self, query: &QueryChild, callback: &mut Option<&mut dyn FnMut(QueryInner)>) -> QueryResult {
        if let Some(inner) = &mut self.0 {
            match query {
                QueryChild::Uid(uid) => {
                    if *uid == FrameUid(&*inner as *const _ as u64) {
                        if let Some(callback) = callback {
                            callback(QueryInner::Child(inner))
                        }
                        else {
                            return QueryResult::child(inner)
                        }
                    }
                }
                QueryChild::Path(path) => if path.len() == 1{
                    if let Some(live_ptr) = inner.query_template(path[0]) {
                        if let Some(callback) = callback {
                            callback(QueryInner::Template(inner, live_ptr))
                        }
                        else {
                            return QueryResult::template(inner, live_ptr)
                        }
                    }
                }
            }
            inner.query_child(query, callback)
        }
        else {
            QueryResult::NotFound
        }
    }
    
    pub fn template(
        &mut self,
        cx: &mut Cx,
        path: &[LiveId],
        new_id: LiveId,
        nodes: &[LiveNode]
    ) -> Option<&mut Box<dyn FrameComponent >> {
        if let Some(inner) = &mut self.0 {
            return inner.template(cx, path, new_id, nodes);
        }
        else {
            None
        }
    }
    
    pub fn draw_component(&mut self, cx: &mut Cx2d, walk: Walk) -> DrawResult {
        if let Some(inner) = &mut self.0 {
            return inner.draw_component(cx, walk, FrameUid(&*inner as *const _ as u64))
        }
        DrawResult::Done
    }
    
    pub fn get_walk(&mut self) -> Walk {
        if let Some(inner) = &mut self.0 {
            return inner.get_walk()
        }
        Walk::default()
    }
    
    // forwarding FrameComponent trait
    pub fn redraw(&mut self, cx: &mut Cx) {
        if let Some(inner) = &mut self.0 {
            return inner.redraw(cx)
        }
    }
    
    pub fn draw_walk_component(&mut self, cx: &mut Cx2d) -> DrawResult {
        if let Some(inner) = &mut self.0 {
            return inner.draw_walk_component(cx, FrameUid(&*inner as *const _ as u64))
        }
        DrawResult::Done
    }
}

impl LiveHook for FrameRef {}
impl LiveApply for FrameRef {
    fn apply(&mut self, cx: &mut Cx, from: ApplyFrom, index: usize, nodes: &[LiveNode]) -> usize {
        if let LiveValue::Class {live_type, ..} = nodes[index].value {
            if let Some(component) = &mut self.0 {
                if component.type_id() != live_type {
                    self.0 = None; // type changed, drop old component
                }
                else {
                    return component.apply(cx, from, index, nodes);
                }
            }
            if let Some(component) = cx.live_registry.clone().borrow()
                .components.get::<FrameComponentRegistry>().new(cx, live_type) {
                self.0 = Some(component);
                return self.0.as_mut().unwrap().apply(cx, from, index, nodes);
            }
        }
        else if let Some(component) = &mut self.0 {
            return component.apply(cx, from, index, nodes)
        }
        nodes.skip_node(index)
    }
}

impl LiveNew for FrameRef {
    fn new(_cx: &mut Cx) -> Self {
        Self (None)
    }
    
    fn live_type_info(_cx: &mut Cx) -> LiveTypeInfo {
        LiveTypeInfo {
            module_id: LiveModuleId::from_str(&module_path!()).unwrap(),
            live_type: LiveType::of::<dyn FrameComponent>(),
            fields: Vec::new(),
            type_name: LiveId(0)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default)]
pub struct FrameUid(u64);

#[derive(Default)]
pub struct FramePath {
    pub uids: Vec<FrameUid>,
    pub ids: Vec<LiveId>
}

impl FramePath {
    pub fn empty() -> Self {
        Self::default()
    }
    pub fn add(mut self, id: LiveId, uid: FrameUid) -> Self {
        self.uids.push(uid);
        self.ids.push(id);
        Self {
            uids: self.uids,
            ids: self.ids
        }
    }
}

pub struct FrameActionItem {
    pub path: FramePath,
    pub action: Box<dyn FrameAction>
}

impl FrameActionItem {
    pub fn id(&self) -> LiveId {
        self.path.ids[0]
    }
}


pub struct DrawStateWrap<T: Clone> {
    state: Option<T>,
    redraw_id: u64
}

impl<T: Clone> Default for DrawStateWrap<T> {
    fn default() -> Self {
        Self {
            state: None,
            redraw_id: 0
        }
    }
}

impl<T: Clone> DrawStateWrap<T> {
    pub fn begin(&mut self, cx: &Cx2d, init: T) -> bool {
        if self.redraw_id != cx.redraw_id {
            self.redraw_id = cx.redraw_id;
            self.state = Some(init);
            true
        }
        else {
            false
        }
    }
    
    pub fn get(&self) -> T {
        self.state.clone().unwrap()
    }
    
    pub fn set(&mut self, value: T) {
        self.state = Some(value);
    }
    
    pub fn end(&mut self) {
        self.state = None;
    }
}

#[macro_export]
macro_rules!frame_component {
    ( $ ty: ty) => {
        | cx: &mut Cx | {
            struct Factory();
            impl FrameComponentFactory for Factory {
                fn new(&self, cx: &mut Cx) -> Box<dyn FrameComponent> {
                    Box::new(< $ ty>::new(cx))
                }
            }
            register_component_factory!(cx, FrameComponentRegistry, $ ty, Factory);
        }
    }
}