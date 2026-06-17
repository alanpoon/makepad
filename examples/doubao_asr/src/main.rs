pub use makepad_widgets;
mod doubao_asr;
use makepad_widgets::*;
app_main!(App);
#[derive(Script, ScriptHook)]
pub struct App {
    #[live] ui: WidgetRef,
}
impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        makepad_widgets::script_mod(vm);
        ScriptValue::NIL
    }
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
