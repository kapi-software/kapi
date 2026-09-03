// 插件路径解析与形态校验测试
// Plugin path resolution and shape validation tests
use tauri_app_lib::plugin::resolve::{
    ensure_entries_exist, is_safe_entry, plan_install, resolve_supported_windows,
};
use tauri_app_lib::plugin::manifest::SupportedWindows;
use std::path::PathBuf;

fn temp_dir_for(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kapi-mgr-test-{}-{}", std::process::id(), tag));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ---- resolve_supported_windows：形态支持解析 / shape-support resolution ----

#[test]
fn resolves_windows_array_with_per_mode_entries() {
    let json = r#"{
        "id": "com.example.demo", "name": "Demo", "version": "1.0.0",
        "windows": [
            { "mode": "embedded", "entry": "index.html" },
            { "mode": "independent", "entry": "window.html", "width": 420, "transparent": true }
        ]
    }"#;
    let s = resolve_supported_windows(json, true, true).unwrap();
    assert_eq!(s.embedded.as_ref().unwrap().entry, "index.html");
    let indep = s.independent.as_ref().unwrap();
    assert_eq!(indep.entry, "window.html");
    assert_eq!(indep.params.width, Some(420.0));
    assert_eq!(indep.params.transparent, Some(true));
    assert!(s.headless); // 有 wasm / has wasm
}

#[test]
fn resolves_legacy_window_fallback() {
    // legacy：window.mode 单形态，入口固定 index.html
    // legacy: a single window.mode shape with the fixed index.html entry
    let s = resolve_supported_windows(
        r#"{"id":"x","name":"X","version":"1","window":{"mode":"independent","width":480}}"#,
        true,
        false,
    )
    .unwrap();
    assert!(s.embedded.is_none());
    assert_eq!(s.independent.as_ref().unwrap().entry, "index.html");
    assert_eq!(s.independent.as_ref().unwrap().params.width, Some(480.0));
    assert!(!s.headless);

    // 未声明 window：缺省 embedded / no window declared: embedded by default
    let s = resolve_supported_windows(r#"{"id":"x","name":"X","version":"1"}"#, true, false).unwrap();
    assert!(s.embedded.is_some());
    assert!(s.independent.is_none());

    // headless-only：无 web → 无窗口形态 / headless-only: no web -> no window shapes
    let s = resolve_supported_windows(
        r#"{"id":"x","name":"X","version":"1","window":{"mode":"headless"}}"#,
        false,
        true,
    )
    .unwrap();
    assert!(s.embedded.is_none());
    assert!(s.independent.is_none());
    assert!(s.headless);
}

#[test]
fn rejects_bad_windows_arrays() {
    // 拼装最小合法前缀 / assemble the minimal valid prefix
    let base = r#"{"id":"x","name":"X","version":"1""#;
    let with = |windows: &str| format!("{base}, \"windows\": [{windows}]}}");

    // headless 不属于 windows[] / headless is not a windows[] mode
    assert!(resolve_supported_windows(
        &with(r#"{"mode":"headless","entry":"index.html"}"#),
        true,
        false
    )
    .is_err());
    // 重复 mode / duplicate mode
    assert!(resolve_supported_windows(
        &with(r#"{"mode":"embedded","entry":"a.html"},{"mode":"embedded","entry":"b.html"}"#),
        true,
        false
    )
    .is_err());
    // 非法 mode / invalid mode
    assert!(resolve_supported_windows(
        &with(r#"{"mode":"popup","entry":"a.html"}"#),
        true,
        false
    )
    .is_err());
}

// ---- plan_install：manifest 校验 / manifest validation ----

// 生成最小合法 manifest（按需覆盖字段；JSON 重复键后者生效；_tail 防尾逗号）
// Minimal valid manifest (override as needed; last duplicate key wins; _tail avoids trailing commas)
fn manifest_json(overrides: &str) -> String {
    format!(
        r#"{{
            "id": "com.example.demo",
            "name": "Demo",
            "version": "1.0.0",
            "author": "Kapi",
            "description": "demo plugin",
            "icon": "icon.png",
            "category": "tool",
            "window": {{"mode": "embedded", "title": "Demo", "width": 420, "height": 640,
                        "minWidth": 320, "minHeight": 400, "resizable": true, "alwaysOnTop": false}},
            {overrides}
            "_tail": true
        }}"#
    )
}

#[test]
fn plans_full_manifest() {
    let plan = plan_install(&manifest_json(""), true, false).unwrap();
    assert_eq!(plan.manifest.id, "com.example.demo");
    assert_eq!(plan.window_mode, "embedded");
    assert_eq!(plan.web_path.as_deref(), Some("web/index.html"));
    assert_eq!(plan.wasm_path, None);
    // window_config 快照保留 camelCase 键
    // The window_config snapshot keeps camelCase keys
    let wc = plan.window_config.unwrap();
    assert!(wc.contains("\"minWidth\""));
    assert!(wc.contains("\"alwaysOnTop\""));
}

#[test]
fn rejects_missing_required_fields() {
    assert!(plan_install(&manifest_json("\"name\": \"\""), true, false).is_err());
    assert!(plan_install(&manifest_json("\"version\": \"\""), true, false).is_err());
    // id 非法字符 / invalid id charset
    assert!(plan_install(&manifest_json("\"id\": \"com foo\""), true, false).is_err());
    assert!(plan_install(&manifest_json("\"id\": \"../x\""), true, false).is_err());
    // __ 前缀保留给宿主共享资源（kapi-plugin:///__kapi__/sdk.js）
    // The __ prefix is reserved for host-shared assets
    assert!(plan_install(&manifest_json("\"id\": \"__kapi__\""), true, false).is_err());
    assert!(plan_install(&manifest_json("\"id\": \"__anything\""), true, false).is_err());
}

#[test]
fn rejects_invalid_json_and_mode() {
    assert!(plan_install("{ not json }", true, false).is_err());
    assert!(plan_install(&manifest_json("\"window\": {\"mode\": \"popup\"}"), true, false).is_err());
}

#[test]
fn rejects_plugin_without_any_entry() {
    assert!(plan_install(&manifest_json(""), false, false).is_err());
}

#[test]
fn derives_mode_from_entries_when_unset() {
    // 未声明 window：有 web → embedded，仅 wasm → headless
    // No window declared: web → embedded, wasm-only → headless
    let no_window = r#"{"id":"com.example.demo","name":"Demo","version":"1.0.0"}"#;
    assert_eq!(plan_install(no_window, true, false).unwrap().window_mode, "embedded");
    assert_eq!(plan_install(no_window, false, true).unwrap().window_mode, "headless");
    let plan = plan_install(no_window, true, false).unwrap();
    assert_eq!(plan.window_config, None);
}

#[test]
fn explicit_mode_wins_over_entries() {
    let headless = r#"{"id":"com.example.demo","name":"Demo","version":"1.0.0","window":{"mode":"headless"}}"#;
    assert_eq!(plan_install(headless, true, false).unwrap().window_mode, "headless");
}

#[test]
fn wasm_entry_recorded() {
    let plan = plan_install(&manifest_json(""), true, true).unwrap();
    assert_eq!(plan.wasm_path.as_deref(), Some("main.wasm"));
}

#[test]
fn parses_tauri_aligned_window_options() {
    // Tauri 对齐窗口选项：camelCase 解析 + window_config 快照保留新键
    // Tauri-aligned window options: camelCase parsing + snapshot keeps the new keys
    let json = r#"{
        "id": "com.example.demo", "name": "Demo", "version": "1.0.0",
        "window": {"mode": "independent", "transparent": true, "decorations": false,
                    "skipTaskbar": true, "shadow": false, "center": false, "fullscreen": false}
    }"#;
    let plan = plan_install(json, true, false).unwrap();
    let w = &plan.manifest.window.unwrap().params;
    assert_eq!(w.transparent, Some(true));
    assert_eq!(w.decorations, Some(false));
    assert_eq!(w.skip_taskbar, Some(true));
    assert_eq!(w.shadow, Some(false));
    assert_eq!(w.center, Some(false));
    assert_eq!(w.fullscreen, Some(false));
    // 快照序列化回 camelCase，前端 PluginWindowConfig 直接可用
    // The snapshot serializes back to camelCase, directly usable by the frontend
    let wc = plan.window_config.unwrap();
    assert!(wc.contains("\"skipTaskbar\""));
    assert!(wc.contains("\"transparent\""));
}

#[test]
fn plan_install_validates_entries_and_default_mode() {
    // entry 文件不存在 / missing entry file（此处纯函数层校验路径安全，存在性见下）
    let missing = r#"{
        "id":"com.example.demo","name":"Demo","version":"1.0.0",
        "windows":[{"mode":"embedded","entry":"../evil.html"}]
    }"#;
    assert!(plan_install(missing, true, false).is_err());
    let slash = r#"{
        "id":"com.example.demo","name":"Demo","version":"1.0.0",
        "windows":[{"mode":"embedded","entry":"/abs.html"}]
    }"#;
    assert!(plan_install(slash, true, false).is_err());

    // 仅声明 independent：默认模式取 independent / independent-only: defaults to independent
    let indep_only = r#"{
        "id":"com.example.demo","name":"Demo","version":"1.0.0",
        "windows":[{"mode":"independent","entry":"window.html","width":300,"transparent":true}]
    }"#;
    let plan = plan_install(indep_only, true, false).unwrap();
    assert_eq!(plan.window_mode, "independent");
    // windows[] 路径的 window_config 快照：independent 形态参数 + mode 键
    // window_config snapshot on the windows[] path: the independent params + mode key
    let wc = plan.window_config.unwrap();
    assert!(wc.contains("\"mode\":\"independent\""));
    assert!(wc.contains("\"width\":300.0"));
    assert!(wc.contains("\"transparent\":true"));

    // 仅 embedded：无 independent 形态则无快照 / embedded-only: no shape, no snapshot
    let embed_only = r#"{
        "id":"com.example.demo","name":"Demo","version":"1.0.0",
        "windows":[{"mode":"embedded","entry":"a.html"}]
    }"#;
    assert_eq!(plan_install(embed_only, true, false).unwrap().window_config, None);
}

// ---- ensure_entries_exist：entry 文件存在性核验 / entry file existence check ----

#[test]
fn ensure_entries_exist_rejects_missing_files() {
    let root = temp_dir_for("entries");
    let src = root.join("src");
    std::fs::create_dir_all(src.join("web")).unwrap();
    std::fs::write(src.join("manifest.json"), "{}").unwrap();
    std::fs::write(src.join("web/index.html"), "<html></html>").unwrap();
    let json = r#"{
        "id":"com.example.demo","name":"Demo","version":"1.0.0",
        "windows":[{"mode":"embedded","entry":"index.html"},{"mode":"independent","entry":"window.html"}]
    }"#;
    let supported = resolve_supported_windows(json, true, false).unwrap();
    // window.html 尚未创建 → 拒绝 / window.html not yet created -> rejected
    assert!(ensure_entries_exist(&src, &supported).is_err());
    std::fs::write(src.join("web/window.html"), "<html></html>").unwrap();
    assert!(ensure_entries_exist(&src, &supported).is_ok());
    let _ = std::fs::remove_dir_all(&root);
}

// ---- is_safe_entry：路径安全校验 / path safety check ----

#[test]
fn is_safe_entry_rejects_unsafe_paths() {
    // 前导斜杠 / leading slash
    assert!(!is_safe_entry("/index.html"));
    // 路径穿越 / path traversal
    assert!(!is_safe_entry("../evil"));
    assert!(!is_safe_entry("foo/../bar"));
    // 空段 / empty segment
    assert!(!is_safe_entry(""));
    assert!(!is_safe_entry("foo//bar"));
    // 非法字符 / invalid chars
    assert!(!is_safe_entry("foo bar.html"));
    assert!(!is_safe_entry("foo<bar>.html"));
}

#[test]
fn is_safe_entry_accepts_valid_paths() {
    assert!(is_safe_entry("index.html"));
    assert!(is_safe_entry("sub/path.html"));
    assert!(is_safe_entry("a.b.c"));
    assert!(is_safe_entry("a_b-c"));
}
