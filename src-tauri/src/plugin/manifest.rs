// 插件 manifest 类型定义：窗口参数、形态、安装计划
// Plugin manifest types: window params, shapes, install plan
use serde::{Deserialize, Serialize};

// 窗口参数（不含 mode）：legacy window 字段与 windows[] 条目共用，对齐 Tauri 窗口选项
// Window params (mode excluded): shared by the legacy window field and windows[] entries
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestWindowParams {
    pub title: Option<String>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub min_width: Option<f64>,
    pub min_height: Option<f64>,
    pub resizable: Option<bool>,
    pub always_on_top: Option<bool>,
    // 透明背景：需窗口与页面双透明；Linux X11 无合成器时退化为黑底
    // Transparent: needs window + page transparency; black on X11 without a compositor
    pub transparent: Option<bool>,
    // 无边框（隐藏标题栏）；默认 true / frameless (hides the title bar); default true
    pub decorations: Option<bool>,
    // 不在任务栏显示；默认 false / hide from the taskbar; default false
    pub skip_taskbar: Option<bool>,
    // 窗口投影（仅 Windows/Linux）；默认 true / shadow (Windows/Linux only); default true
    pub shadow: Option<bool>,
    // 居中创建；默认 true / center on creation; default true
    pub center: Option<bool>,
    pub fullscreen: Option<bool>,
}

// manifest.window：legacy 单形态声明（mode 与参数扁平同层；缺省字段由启动时回退默认值）
// manifest.window: the legacy single-shape declaration (mode flattened with params)
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestWindow {
    pub mode: Option<String>,
    #[serde(flatten)]
    pub params: ManifestWindowParams,
}

// manifest.windows[]：多形态声明（mode + entry + 参数）；entry 相对 web/，如 "index.html"
// manifest.windows[]: multi-shape declaration (mode + entry + params); entry is web/-relative
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestWindowEntry {
    pub mode: Option<String>,
    pub entry: Option<String>,
    #[serde(flatten)]
    pub params: ManifestWindowParams,
}

// manifest.json：安装校验所需字段（kapi_version / workflow / permissions 原样入库）
// manifest.json: fields needed for install validation (other keys stored verbatim)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub category: Option<String>,
    pub window: Option<ManifestWindow>,
    pub windows: Option<Vec<ManifestWindowEntry>>,
}

// 解析后的单个形态：入口文件（相对 web/）+ 窗口参数
// One resolved shape: the entry file (web/-relative) plus window params
#[derive(Debug, Default, Clone)]
pub struct ResolvedWindow {
    pub entry: String,
    pub params: ManifestWindowParams,
}

// 插件声明支持的形态：windows 数组优先，legacy window 回退；headless = 有 wasm 入口
// The plugin's declared shapes: the windows array wins, the legacy window field is the
// fallback; headless support equals having a wasm entry
#[derive(Debug, Default)]
pub struct SupportedWindows {
    pub embedded: Option<ResolvedWindow>,
    pub independent: Option<ResolvedWindow>,
    pub headless: bool,
}

// 安装计划：manifest 校验与入口推导的纯函数产物（无 IO，便于单测）
// Install plan: pure-function result of manifest validation and entry derivation (no IO)
#[derive(Debug)]
pub struct InstallPlan {
    pub manifest: Manifest,
    pub manifest_json: String,
    pub window_mode: String,
    pub window_config: Option<String>,
    pub web_path: Option<String>,
    pub wasm_path: Option<String>,
}