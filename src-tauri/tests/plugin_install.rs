// 插件目录复制与窗口 label 映射测试
// Plugin directory copy and window label mapping tests
use std::path::PathBuf;
use tauri_app_lib::plugin::install::{copy_dir_recursive, plugin_window_label};

fn temp_dir_for(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kapi-mgr-test-{}-{}", std::process::id(), tag));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ---- copy_dir_recursive：目录复制 / dir copying ----

#[test]
fn copies_nested_tree() {
    let root = temp_dir_for("copy");
    let src = root.join("src");
    std::fs::create_dir_all(src.join("web/assets")).unwrap();
    std::fs::write(src.join("manifest.json"), "{}").unwrap();
    std::fs::write(src.join("web/index.html"), "<html>hi</html>").unwrap();
    std::fs::write(src.join("web/assets/app.js"), "console.log(1)").unwrap();

    let dst = root.join("dst");
    copy_dir_recursive(&src, &dst).unwrap();

    assert_eq!(
        std::fs::read_to_string(dst.join("web/index.html")).unwrap(),
        "<html>hi</html>"
    );
    assert_eq!(
        std::fs::read_to_string(dst.join("web/assets/app.js")).unwrap(),
        "console.log(1)"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn copy_fails_for_missing_source() {
    let root = temp_dir_for("copy-missing");
    assert!(copy_dir_recursive(&root.join("nope"), &root.join("dst")).is_err());
    let _ = std::fs::remove_dir_all(&root);
}

// ---- plugin_window_label：label 字符映射 / label character mapping ----

#[test]
fn window_label_dots_are_sanitized() {
    // Tauri label 禁止 "."：反向域名 id 必须映射为合法字符
    // Tauri labels forbid ".": reverse-domain ids must map to legal characters
    assert_eq!(
        plugin_window_label("com.kapi.sample.plugin-a"),
        "plugin-com_kapi_sample_plugin-a"
    );
    // 同一 id 重复计算结果一致（聚焦已有窗口依赖确定性）
    // Repeated computation stays stable (focusing relies on determinism)
    assert_eq!(
        plugin_window_label("com.kapi.sample.plugin-a"),
        plugin_window_label("com.kapi.sample.plugin-a")
    );
    // 无点 id 原样保留
    // Dot-free ids pass through unchanged
    assert_eq!(plugin_window_label("simple_id-1"), "plugin-simple_id-1");
}