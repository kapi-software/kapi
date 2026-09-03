// headless 默认动作选择逻辑测试
// Headless default action selection logic tests
use tauri_app_lib::plugin::pool::pick_headless_action;

#[test]
fn headless_action_prefers_run_then_first() {
    // run 优先 / a declared run wins
    let with_run = r#"{"workflow":{"actions":[{"name":"format"},{"name":"run"}]}}"#;
    assert_eq!(pick_headless_action(with_run), "run");
    // 无 run → 首个 action / no run -> the first action
    let no_run = r#"{"workflow":{"actions":[{"name":"format"},{"name":"save"}]}}"#;
    assert_eq!(pick_headless_action(no_run), "format");
    // 无 workflow / actions → 字面量 "run" / no workflow or actions -> the literal "run"
    assert_eq!(pick_headless_action(r#"{"id":"x"}"#), "run");
    assert_eq!(pick_headless_action("{ bad json"), "run");
}
