// 插件路径解析与形态支持校验（纯函数）
// Plugin path resolution and shape-support validation (pure functions)
use std::path::Path;

use crate::plugin::manifest::{Manifest, ResolvedWindow, SupportedWindows};
use crate::plugin_protocol::is_valid_plugin_id;

// 合法运行模式（docs/PLUGINS.md §2.1）
// Valid window modes (docs/PLUGINS.md §2.1)
const MODES: [&str; 3] = ["embedded", "independent", "headless"];

// 形态支持解析（纯函数）：windows[] 逐条入位（每 mode 至多一条）；无数组时按 legacy
// window.mode（缺省 embedded）单形态、入口固定 index.html；无 web 入口则无窗口形态
// Shape resolution (pure): windows[] entries slot in (at most one per mode); without the
// array the legacy window.mode (default embedded) yields a single index.html shape;
// no web entry means no window shapes at all
pub fn resolve_supported_windows(
    manifest_json: &str,
    has_web: bool,
    has_wasm: bool,
) -> Result<SupportedWindows, String> {
    let manifest: Manifest = serde_json::from_str(manifest_json)
        .map_err(|e| format!("manifest.json 解析失败 / invalid manifest.json: {e}"))?;
    let mut out = SupportedWindows { headless: has_wasm, ..Default::default() };

    if let Some(entries) = manifest.windows {
        for entry in entries {
            let mode = entry.mode.clone().unwrap_or_else(|| "embedded".into());
            let resolved = ResolvedWindow {
                entry: entry.entry.clone().unwrap_or_else(|| "index.html".into()),
                params: entry.params,
            };
            match mode.as_str() {
                "embedded" if out.embedded.is_none() => out.embedded = Some(resolved),
                "independent" if out.independent.is_none() => out.independent = Some(resolved),
                // headless 由 main.wasm 决定，不进 windows[] / headless comes from main.wasm only
                "headless" => {
                    return Err("windows[] 不支持 headless（由 main.wasm 决定）/ headless is not a windows[] mode (decided by main.wasm)".into())
                }
                other => {
                    return Err(format!(
                        "windows[] mode 非法或重复 / invalid or duplicate windows[] mode: {other}"
                    ))
                }
            }
        }
    } else {
        // legacy：单一形态（mode 缺省 embedded），入口固定 index.html
        // legacy: a single shape (mode defaults to embedded) with the fixed index.html entry
        let window = manifest.window.unwrap_or_default();
        let resolved =
            ResolvedWindow { entry: "index.html".into(), params: window.params };
        match window.mode.as_deref().unwrap_or("embedded") {
            "embedded" => out.embedded = Some(resolved),
            "independent" => out.independent = Some(resolved),
            // headless 声明不产生窗口形态 / a headless declaration yields no window shape
            _ => {}
        }
    }

    // headless-only（无 web 入口）：两种窗口形态都不存在
    // headless-only (no web entry): neither window shape exists
    if !has_web {
        out.embedded = None;
        out.independent = None;
    }
    Ok(out)
}

// entry 文件存在性核验：windows[] 每个形态的入口必须真实存在（命令侧，plan_install 保持纯函数）
// Entry file existence: every declared shape's entry must exist (command-side; plan_install stays pure)
pub fn ensure_entries_exist(src: &Path, supported: &SupportedWindows) -> Result<(), String> {
    for resolved in [&supported.embedded, &supported.independent].into_iter().flatten() {
        if !src.join("web").join(&resolved.entry).is_file() {
            return Err(format!(
                "windows[].entry 文件不存在 / missing entry file: web/{}",
                resolved.entry
            ));
        }
    }
    Ok(())
}

// entry 路径安全（相对 web/）：非空、无前导 /、每段仅 [A-Za-z0-9._-]（URL 安全）
// Entry path safety (web/-relative): non-empty, no leading slash, slug segments (URL-safe)
pub fn is_safe_entry(entry: &str) -> bool {
    !entry.is_empty()
        && !entry.starts_with('/')
        && entry.split('/').all(|seg| {
            !seg.is_empty()
                && seg != ".."
                && seg != "."
                && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
        })
}

// 安装计划校验（plan_install）：校验 manifest 并推导安装计划
// Validate and derive install plan: checks manifest and derives the install plan
pub fn plan_install(
    manifest_json: &str,
    has_web: bool,
    has_wasm: bool,
) -> Result<crate::plugin::manifest::InstallPlan, String> {
    let manifest: Manifest = serde_json::from_str(manifest_json)
        .map_err(|e| format!("manifest.json 解析失败 / invalid manifest.json: {e}"))?;

    if !is_valid_plugin_id(&manifest.id) {
        return Err(format!(
            "插件 id 非法（仅限 [A-Za-z0-9._-]）/ invalid plugin id: {}",
            manifest.id
        ));
    }
    // __ 前缀保留给宿主共享资源（kapi-plugin:///__kapi__/sdk.js），插件不得占用
    // The __ prefix is reserved for host-shared assets (kapi-plugin:///__kapi__/sdk.js)
    if manifest.id.starts_with("__") {
        return Err(format!(
            "插件 id 保留（__ 前缀属宿主）/ reserved plugin id: {}",
            manifest.id
        ));
    }
    if manifest.name.trim().is_empty() {
        return Err("manifest 缺少 name / manifest is missing name".into());
    }
    if manifest.version.trim().is_empty() {
        return Err("manifest 缺少 version / manifest is missing version".into());
    }
    if let Some(mode) = manifest.window.as_ref().and_then(|w| w.mode.as_deref()) {
        if !MODES.contains(&mode) {
            return Err(format!("非法 window.mode / invalid window.mode: {mode}"));
        }
    }
    if !has_web && !has_wasm {
        return Err(
            "插件缺少入口（web/index.html 或 main.wasm 至少其一）/ plugin has no entry (web/index.html or main.wasm)"
                .into(),
        );
    }

    // 形态支持解析 + windows[] 校验（mode 白名单/唯一、entry 路径安全；文件存在性由调用侧核验）
    // Shape resolution plus windows[] validation (whitelisted/unique modes; path-safe entries —
    // file existence is the caller's check since plan_install stays pure)
    let supported = resolve_supported_windows(manifest_json, has_web, has_wasm)?;
    for resolved in [&supported.embedded, &supported.independent].into_iter().flatten() {
        if !is_safe_entry(&resolved.entry) {
            return Err(format!(
                "windows[].entry 非法（每段仅限 [A-Za-z0-9._-]）/ invalid windows entry: {}",
                resolved.entry
            ));
        }
    }

    // 运行模式：legacy window.mode 显式声明优先，否则按支持形态取默认（embedded 优先）
    // Window mode: an explicit legacy window.mode wins; otherwise default from the
    // supported shapes (embedded first)
    let window_mode = match manifest.window.as_ref().and_then(|w| w.mode.clone()) {
        Some(mode) => mode,
        None => if supported.embedded.is_some() {
            "embedded"
        } else if supported.independent.is_some() {
            "independent"
        } else {
            "headless"
        }
        .to_string(),
    };

    // window_config 快照：independent 形态的窗口参数（windows[] 条目优先，legacy window 回退）。
    // 独立窗口壳（PluginWindowShell）读它做首帧透明判定，windows[] 插件不能缺失
    // window_config snapshot: the independent shape's params (windows[] entry first,
    // legacy window fallback). The independent shell (PluginWindowShell) reads it for the
    // first-frame transparency decision, so windows[] plugins must not miss it
    let window_config = match supported.independent.as_ref() {
        Some(indep) => {
            // windows[] 路径：参数 + mode 键，保持前端 PluginWindowConfig 形状
            // windows[] path: params plus the mode key, matching the frontend PluginWindowConfig shape
            use serde_json::Value;
            let mut v = serde_json::to_value(&indep.params).map_err(|e| e.to_string())?;
            if let Value::Object(map) = &mut v {
                map.insert("mode".into(), Value::String("independent".into()));
            }
            Some(v.to_string())
        }
        // legacy 路径：整个 window 原样快照（含 mode）
        // legacy path: snapshot the whole window verbatim (mode included)
        None => manifest
            .window
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| e.to_string())?,
    };

    Ok(crate::plugin::manifest::InstallPlan {
        manifest_json: manifest_json.to_string(),
        window_mode,
        window_config,
        web_path: if has_web {
            Some("web/index.html".into())
        } else {
            None
        },
        wasm_path: if has_wasm { Some("main.wasm".into()) } else { None },
        manifest,
    })
}