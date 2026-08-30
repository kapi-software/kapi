// kapi-plugin:// 自定义协议：插件 web/ 静态资源服务（含路径安全）
// kapi-plugin:// custom protocol: static file serving from plugin web/ (path-safe)
use std::borrow::Cow;
use std::path::Path;

use tauri::http::{Request, Response};
use tauri::{AppHandle, Manager, Runtime};

// 协议名：与前端 URL 构造（src/lib/plugin-url.ts）必须保持一致
// Protocol name: must stay in sync with the frontend URL builder (src/lib/plugin-url.ts)
pub const SCHEME: &str = "kapi-plugin";

// 保留段：宿主共享资源的命名空间（__ 前缀，插件 id 禁用），当前承载 @kapi/plugin-sdk
// Reserved segment: the host-shared asset namespace (__ prefix, banned for plugin ids);
// currently serves @kapi/plugin-sdk
const SDK_NAMESPACE: &str = "__kapi__";
// SDK 单文件：构建期内嵌，随宿主版本更新（插件页以绝对路径 /__kapi__/sdk.js 引用）
// The single-file SDK: embedded at build time and versioned with the host (plugin pages
// reference it by the absolute path /__kapi__/sdk.js)
const SDK_JS: &str = include_str!("../assets/kapi-sdk.js");

// 请求解析错误：统一映射为 403/404，响应体不携带任何文件系统细节
// Request parse errors: mapped to 403/404; response bodies never leak fs details
#[derive(Debug)]
enum ProtocolError {
    // 请求本身非法（穿越 / 非法字符 / 非法转义）→ 403
    // The request itself is invalid (traversal / bad charset / bad escape) -> 403
    Forbidden,
    // 资源不存在（空路径 / 文件缺失）→ 404
    // Resource missing (empty path / absent file) -> 404
    NotFound,
}

// 协议入口：lib.rs 注册的处理器，资源根固定为 {app_data}/plugins
// Protocol entry: the handler registered in lib.rs; asset root is {app_data}/plugins
pub fn handle<R: Runtime>(
    app: &AppHandle<R>,
    request: Request<Vec<u8>>,
) -> Response<Cow<'static, [u8]>> {
    let plugins_root = match app.path().app_data_dir() {
        Ok(dir) => dir.join("plugins"),
        Err(_) => return error_response(500, "Internal Server Error"),
    };
    serve_from(
        &plugins_root,
        request.method().as_str(),
        request.uri().path(),
    )
}

// 服务单条请求：方法门禁 → 词法校验 → canonicalize 前缀约束 → 读文件
// Serve one request: method gate -> lexical checks -> canonicalize prefix constraint -> read file
fn serve_from(plugins_root: &Path, method: &str, uri_path: &str) -> Response<Cow<'static, [u8]>> {
    // 静态资源协议只接受 GET（iframe/fetch 加载资源均走 GET）
    // The static protocol accepts GET only (iframe/fetch asset loads are all GET)
    if method != "GET" {
        return error_response(405, "Method Not Allowed");
    }

    let (plugin_id, rel) = match parse_request_path(uri_path) {
        Ok(v) => v,
        Err(ProtocolError::Forbidden) => return error_response(403, "Forbidden"),
        Err(ProtocolError::NotFound) => return error_response(404, "Not Found"),
    };

    // 保留命名空间：仅 /__kapi__/sdk.js 一个资源，其余一律 404（不落入插件目录查找）
    // Reserved namespace: exactly one asset /__kapi__/sdk.js; anything else is a 404
    // (never resolved against plugin directories)
    if plugin_id == SDK_NAMESPACE {
        if rel == ["sdk.js"] {
            return Response::builder()
                .status(200)
                .header("Content-Type", "text/javascript; charset=utf-8")
                .header("Cache-Control", "no-store")
                .body(Cow::Borrowed(SDK_JS.as_bytes()))
                .unwrap();
        }
        return error_response(404, "Not Found");
    }

    // web 根目录：{plugins_root}/{id}/web —— 协议只暴露该子目录，main.wasm 等不可达
    // Web root: {plugins_root}/{id}/web - the only exposed subtree; main.wasm etc. stay unreachable
    let web_root = plugins_root.join(&plugin_id).join("web");
    let mut candidate = web_root.clone();
    for seg in &rel {
        candidate.push(seg);
    }

    // 双保险：词法校验之外，canonicalize 解析符号链接后必须仍位于 web 根内
    // Belt-and-suspenders: past lexical checks, the symlink-resolved path must stay inside the web root
    let canonical_root = match web_root.canonicalize() {
        Ok(p) => p,
        Err(_) => return error_response(404, "Not Found"),
    };
    let canonical = match candidate.canonicalize() {
        Ok(p) => p,
        Err(_) => return error_response(404, "Not Found"),
    };
    if !canonical.starts_with(&canonical_root) {
        // 可疑请求：词法合法但经符号链接逃出 web 根，拒绝并留痕
        // Suspicious request: lexically valid but escapes the web root via symlink; reject and log
        eprintln!(
            "kapi-plugin: blocked symlink escape for plugin '{}' ({})",
            plugin_id, uri_path
        );
        return error_response(403, "Forbidden");
    }
    if !canonical.is_file() {
        return error_response(404, "Not Found");
    }

    match std::fs::read(&canonical) {
        Ok(body) => file_response(&canonical, body),
        Err(_) => error_response(404, "Not Found"),
    }
}

// plugin_id 合法性：字符集 [A-Za-z0-9._-]，整体不能是 "." 或 ".."
// plugin_id validity: charset [A-Za-z0-9._-]; "." or ".." as a whole are rejected
// 协议与插件安装校验（plugin_manager.rs）共用同一份规则
// Shared by the protocol and install validation (plugin_manager.rs)
pub fn is_valid_plugin_id(id: &str) -> bool {
    !id.is_empty()
        && id != "."
        && id != ".."
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

// 请求路径 → (plugin_id, web 内相对路径段)：百分号解码 + 词法安全校验
// Request path -> (plugin_id, relative segments under web/): percent-decode + lexical safety checks
fn parse_request_path(uri_path: &str) -> Result<(String, Vec<String>), ProtocolError> {
    let decoded = percent_decode(uri_path)?;
    let decoded = String::from_utf8(decoded).map_err(|_| ProtocolError::Forbidden)?;

    // 反斜杠一律拒绝：合法 Web 资源不会使用，且它是 Windows 下的穿越向量
    // Reject backslashes outright: legit web assets never use them; on Windows they enable traversal
    if decoded.contains('\\') {
        return Err(ProtocolError::Forbidden);
    }

    let mut segments: Vec<&str> = decoded.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return Err(ProtocolError::NotFound);
    }
    let plugin_id = segments.remove(0);
    if !is_valid_plugin_id(plugin_id) {
        return Err(ProtocolError::Forbidden);
    }

    // 相对路径段：拒绝 ".."，丢弃 "."；空（仅插件根）回退 index.html
    // Relative segments: reject "..", drop "."; empty (plugin root only) falls back to index.html
    let mut rel: Vec<String> = Vec::new();
    for seg in &segments {
        match *seg {
            ".." => return Err(ProtocolError::Forbidden),
            "." => continue,
            s => rel.push(s.to_string()),
        }
    }
    if rel.is_empty() {
        rel.push("index.html".to_string());
    }
    Ok((plugin_id.to_string(), rel))
}

// 百分号解码：%XX → 字节；非法转义或解码后非 UTF-8 一律拒绝
// Percent-decode: %XX -> byte; malformed escapes or non-UTF-8 results are rejected
fn percent_decode(input: &str) -> Result<Vec<u8>, ProtocolError> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            // 转义必须恰好跟随两个十六进制字符，否则视为非法请求
            // An escape must be followed by exactly two hex digits, else the request is invalid
            if i + 2 >= bytes.len() {
                return Err(ProtocolError::Forbidden);
            }
            let hi = hex_val(bytes[i + 1]).ok_or(ProtocolError::Forbidden)?;
            let lo = hex_val(bytes[i + 2]).ok_or(ProtocolError::Forbidden)?;
            out.push(hi * 16 + lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Ok(out)
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

// 扩展名 → Content-Type（文本类附 charset）；未知类型回退 octet-stream
// Extension -> Content-Type (charset for text); unknown types fall back to octet-stream
fn content_type(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "wasm" => "application/wasm",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

// 成功响应：Content-Type + no-store（插件更新后不残留旧资源缓存）
// Success response: Content-Type + no-store (no stale asset cache after plugin updates)
fn file_response(path: &Path, body: Vec<u8>) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(200)
        .header("Content-Type", content_type(path))
        .header("Cache-Control", "no-store")
        .body(Cow::Owned(body))
        .unwrap()
}

// 错误响应：固定文案，不回显请求路径或目标位置
// Error response: fixed text, never echoing the request path or target location
fn error_response(status: u16, text: &'static str) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .body(Cow::Borrowed(text.as_bytes()))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // ---- parse_request_path：词法安全 / lexical safety ----

    #[test]
    fn parses_normal_path() {
        let (id, rel) = parse_request_path("/com.example.foo/index.html").unwrap();
        assert_eq!(id, "com.example.foo");
        assert_eq!(rel, vec!["index.html"]);
    }

    #[test]
    fn plugin_root_falls_back_to_index() {
        // 插件根（无尾路径 / 带尾斜杠）都回退 index.html
        // Plugin root (no rest / trailing slash) both fall back to index.html
        for path in ["/com.example.foo", "/com.example.foo/"] {
            let (id, rel) = parse_request_path(path).unwrap();
            assert_eq!(id, "com.example.foo");
            assert_eq!(rel, vec!["index.html"]);
        }
    }

    #[test]
    fn keeps_nested_segments_in_order() {
        let (_, rel) = parse_request_path("/com.foo/assets/app.main.js").unwrap();
        assert_eq!(rel, vec!["assets", "app.main.js"]);
    }

    #[test]
    fn drops_dot_segments() {
        let (_, rel) = parse_request_path("/com.foo/./assets/./app.js").unwrap();
        assert_eq!(rel, vec!["assets", "app.js"]);
    }

    #[test]
    fn rejects_plain_traversal() {
        assert!(matches!(
            parse_request_path("/com.foo/../other/web/x"),
            Err(ProtocolError::Forbidden)
        ));
    }

    #[test]
    fn rejects_encoded_traversal_in_id() {
        // %2e%2e 解码后即为 ".."，必须当作穿越拒绝
        // %2e%2e decodes to ".." and must be rejected as traversal
        assert!(matches!(
            parse_request_path("/%2e%2e/x"),
            Err(ProtocolError::Forbidden)
        ));
    }

    #[test]
    fn rejects_encoded_traversal_in_rel() {
        assert!(matches!(
            parse_request_path("/com.foo/%2E%2E/x"),
            Err(ProtocolError::Forbidden)
        ));
    }

    #[test]
    fn rejects_double_encoded_traversal() {
        // %252e 仅解码一层为 "%2e"，非法字符 '%' 被 plugin_id 字符集拒绝
        // %252e single-decodes to "%2e"; the invalid '%' fails the plugin_id charset
        assert!(matches!(
            parse_request_path("/%252e%252e/x"),
            Err(ProtocolError::Forbidden)
        ));
    }

    #[test]
    fn rejects_backslash_anywhere() {
        assert!(matches!(
            parse_request_path("/com.foo/a\\b"),
            Err(ProtocolError::Forbidden)
        ));
    }

    #[test]
    fn rejects_invalid_plugin_id_charset() {
        for path in ["/com foo/x", "/com%25foo/x", "/com~foo/x"] {
            assert!(matches!(parse_request_path(path), Err(ProtocolError::Forbidden)));
        }
    }

    #[test]
    fn accepts_common_plugin_id_charset() {
        for path in ["/com.example.code-beautifier/x", "/a_b.C-d/x", "/x/y"] {
            assert!(parse_request_path(path).is_ok());
        }
    }

    #[test]
    fn empty_path_is_not_found() {
        assert!(matches!(parse_request_path("/"), Err(ProtocolError::NotFound)));
    }

    #[test]
    fn rejects_malformed_percent_escape() {
        assert!(matches!(
            parse_request_path("/com.foo/%zz"),
            Err(ProtocolError::Forbidden)
        ));
    }

    #[test]
    fn rejects_non_utf8_decoded_bytes() {
        // %ff 无法构成合法 UTF-8，拒绝
        // %ff cannot form valid UTF-8; rejected
        assert!(matches!(
            parse_request_path("/com.foo/%ff"),
            Err(ProtocolError::Forbidden)
        ));
    }

    #[test]
    fn decodes_percent_in_rel_path() {
        // 合法百分号解码（连字符）应生效
        // Legit percent-decoding (hyphen) works
        let (id, _) = parse_request_path("/com.example.my%2Dplugin/index.html").unwrap();
        assert_eq!(id, "com.example.my-plugin");
    }

    // ---- serve_from：文件服务 / file serving ----

    // 构造一次性临时插件根目录（测试各自独享，互不干扰）
    // Build a throwaway temp plugins root (one per test, no cross-test interference)
    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kapi-proto-test-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_file(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn serves_index_html_with_headers() {
        let root = temp_root("index");
        write_file(&root, "com.foo/web/index.html", "<html>hello</html>");

        let res = serve_from(&root, "GET", "/com.foo/index.html");
        assert_eq!(res.status(), 200);
        assert_eq!(
            res.headers().get("Content-Type").unwrap(),
            "text/html; charset=utf-8"
        );
        // no-store：插件更新后不使用旧缓存
        // no-store: no stale cache after plugin updates
        assert_eq!(res.headers().get("Cache-Control").unwrap(), "no-store");
        assert_eq!(res.body().as_ref(), b"<html>hello</html>".as_slice());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn plugin_root_serves_index_fallback() {
        let root = temp_root("root-fallback");
        write_file(&root, "com.foo/web/index.html", "<html>x</html>");

        let res = serve_from(&root, "GET", "/com.foo/");
        assert_eq!(res.status(), 200);
        assert_eq!(res.body().as_ref(), b"<html>x</html>".as_slice());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn serves_nested_js_with_content_type() {
        let root = temp_root("nested");
        write_file(&root, "com.foo/web/assets/app.js", "console.log(1)");

        let res = serve_from(&root, "GET", "/com.foo/assets/app.js");
        assert_eq!(res.status(), 200);
        assert_eq!(
            res.headers().get("Content-Type").unwrap(),
            "text/javascript; charset=utf-8"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_extension_falls_back_to_octet_stream() {
        let root = temp_root("octet");
        write_file(&root, "com.foo/web/data.bin", "raw");

        let res = serve_from(&root, "GET", "/com.foo/data.bin");
        assert_eq!(res.status(), 200);
        assert_eq!(
            res.headers().get("Content-Type").unwrap(),
            "application/octet-stream"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_file_or_plugin_is_not_found() {
        let root = temp_root("missing");
        write_file(&root, "com.foo/web/index.html", "x");

        // 文件缺失 → 404
        // Absent file -> 404
        assert_eq!(serve_from(&root, "GET", "/com.foo/nope.html").status(), 404);
        // 整个插件未安装 → 404
        // Plugin not installed at all -> 404
        assert_eq!(serve_from(&root, "GET", "/com.bar/index.html").status(), 404);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn never_serves_files_outside_web_dir() {
        let root = temp_root("outside-web");
        write_file(&root, "com.foo/web/index.html", "safe");
        // manifest.json 与 main.wasm 位于 web/ 之外，协议必须拒绝访问
        // manifest.json and main.wasm sit outside web/; the protocol must refuse them
        write_file(&root, "com.foo/manifest.json", "{}");
        write_file(&root, "com.foo/main.wasm", "wasm-bytes");

        // 词法层即拒绝：web 之外的路径无法用 ".." 表达（403）
        // Rejected lexically: paths outside web/ cannot be expressed with ".." (403)
        assert_eq!(serve_from(&root, "GET", "/com.foo/../com.foo/manifest.json").status(), 403);
        // 直接拼出的越界文件名不存在 → 404（不泄露存在性）
        // A directly-joined out-of-tree name does not exist -> 404 (no existence leak)
        assert_eq!(serve_from(&root, "GET", "/com.foo/manifest.json").status(), 404);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_non_get_methods() {
        let root = temp_root("method");
        write_file(&root, "com.foo/web/index.html", "x");

        assert_eq!(serve_from(&root, "POST", "/com.foo/index.html").status(), 405);
        assert_eq!(serve_from(&root, "HEAD", "/com.foo/index.html").status(), 405);

        let _ = std::fs::remove_dir_all(&root);
    }

    // ---- 保留命名空间 / reserved namespace ----

    #[test]
    fn serves_reserved_sdk_asset() {
        // SDK 不依赖任何插件目录（根目录为空也能服务）
        // The SDK never touches plugin directories (an empty root still serves it)
        let root = temp_root("sdk");
        let res = serve_from(&root, "GET", "/__kapi__/sdk.js");
        assert_eq!(res.status(), 200);
        assert_eq!(
            res.headers().get("Content-Type").unwrap(),
            "text/javascript; charset=utf-8"
        );
        let body = String::from_utf8_lossy(res.body().as_ref()).to_string();
        assert!(body.starts_with("// @kapi/plugin-sdk"));
        assert!(body.contains("kapi:events.on"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reserved_namespace_rejects_other_paths() {
        let root = temp_root("sdk-404");
        // 保留段下仅 sdk.js 一个资源；其余（含目录回退）一律 404
        // Exactly one asset under the reserved segment; anything else (including the
        // index fallback) is a 404
        assert_eq!(serve_from(&root, "GET", "/__kapi__/other.js").status(), 404);
        assert_eq!(serve_from(&root, "GET", "/__kapi__/").status(), 404);
        assert_eq!(serve_from(&root, "GET", "/__kapi__/../com.foo/x").status(), 403);

        let _ = std::fs::remove_dir_all(&root);
    }

    // 符号链接逃逸仅在 Unix 可构造（Windows 需特权）；canonicalize 前缀约束在此验证
    // Symlink escapes can only be constructed on Unix (Windows needs privileges); the canonicalize prefix check is verified here
    #[cfg(unix)]
    #[test]
    fn blocks_symlink_escape() {
        let root = temp_root("symlink");
        write_file(&root, "secret.txt", "secret");
        write_file(&root, "com.foo/web/index.html", "safe");
        std::os::unix::fs::symlink(root.join("secret.txt"), root.join("com.foo/web/leak.txt"))
            .unwrap();

        assert_eq!(serve_from(&root, "GET", "/com.foo/leak.txt").status(), 403);

        let _ = std::fs::remove_dir_all(&root);
    }
}
