// 插件市场：GitHub 目录源浏览与安装/更新（docs/PLUGINS.md §7）
// Plugin store: browsing and install/update from a GitHub directory source
// 源格式：仓库顶层每个目录 = 一个插件（内含 manifest.json）；市场安装与本地导入共用
// install_from_dir 核心（allow_update = 更新语义，保留启停/排序）
// Source format: every top-level repo dir is one plugin (with manifest.json inside);
// the store shares the install_from_dir core with local import (allow_update keeps
// enable state and ordering)
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use serde_json::Value;
use tauri::AppHandle;

use crate::plugin::install::install_from_dir;

// 市场边界：目录数 / zip 条目数 / 解压总量 / 下载体大小（防滥用与 zip 炸弹）
// Store bounds: dir count / zip entry count / total uncompressed size / download cap
const MAX_LISTED_PLUGINS: usize = 60;
const MAX_ZIP_ENTRIES: usize = 2000;
const MAX_ZIP_TOTAL_BYTES: u64 = 200 * 1024 * 1024;
const MAX_DOWNLOAD_BYTES: usize = 100 * 1024 * 1024;

// 源仓库标识校验：恰好一段 "/"，两段均为 [A-Za-z0-9._-]（防 URL/头注入）
// Source repo validation: exactly one "/", both segments [A-Za-z0-9._-] (no URL/header
// injection)
// is_valid_repo("kapi-plugins/kapi-plugins") => true; is_valid_repo("../x") => false
pub(crate) fn is_valid_repo(repo: &str) -> bool {
    let parts: Vec<&str> = repo.split('/').collect();
    parts.len() == 2
        && parts.iter().all(|p| {
            !p.is_empty()
                && *p != "."
                && *p != ".."
                && p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        })
}

// 市场专用 HTTP 客户端：跟随重定向（zipball 302 → codeload），UA 为 GitHub 必填
// Store-specific client: follows redirects (zipball 302 -> codeload); the UA is required by GitHub
fn store_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(concat!("Kapi-Store/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build the store http client")
    })
}

// 插件目录名与仓库标识同字符集（zip 子树选择的关键，防穿越）
// Plugin dir names share the repo-identifier charset (key to subtree selection; no traversal)
pub(crate) fn is_valid_dir_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

// 市场索引 URL：kapi-plugins 仓库的静态 index.json（raw.githubusercontent 自带 CDN 缓存）；
// Cloudflare Worker 上线后仅需替换此常量，JSON 契约不变
// Store index URL: the static index.json in the kapi-plugins repo (raw.githubusercontent
// is CDN-cached already); once the Cloudflare Worker lands only this constant changes —
// the JSON contract stays
const STORE_INDEX_URL: &str =
    "https://raw.githubusercontent.com/kapi-software/kapi-plugins/HEAD/index.json";

// 本地缓存键（settings 表，值为索引原文；打开市场页先读缓存，手动刷新才回源）
// Local cache key (settings table, the verbatim index body; the store page reads the
// cache first and only a manual refresh hits the source)
const STORE_CACHE_KEY: &str = "store.index";

// 索引条目（index.json plugins[] 元素；serde 直接反序列化 + 校验）
// Index entry (one plugins[] element of index.json; deserialized and validated)
#[derive(serde::Deserialize, serde::Serialize)]
struct StoreIndexEntry {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    category: Option<String>,
    // 插件独立仓库 owner/name（或承载它的索引仓库）
    // The plugin's own repo as owner/name (or the index repo hosting it)
    repo: String,
    // 仓库内插件目录；缺省 = 仓库根即插件包
    // In-repo plugin dir; a missing value means the repo root is the plugin package
    #[serde(default)]
    dir: Option<String>,
}

// 索引解析（纯函数）：结构校验 + 条目过滤（repo/dir 非法直接丢弃，不拖垮整表）
// Index parsing (pure): structural validation; invalid repo/dir entries are dropped
// without sinking the whole listing
fn parse_store_index(body: &str) -> Result<Vec<StoreIndexEntry>, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| format!("InvalidPayload: bad index json ({e})"))?;
    let Some(arr) = v.get("plugins").and_then(Value::as_array) else {
        return Err("InvalidPayload: index json has no plugins array".into());
    };
    let mut out = Vec::new();
    for item in arr.iter().take(MAX_LISTED_PLUGINS) {
        let Ok(entry) = serde_json::from_value::<StoreIndexEntry>(item.clone()) else {
            continue;
        };
        if entry.id.trim().is_empty() || !is_valid_repo(&entry.repo) {
            continue;
        }
        if let Some(dir) = &entry.dir {
            if !is_valid_dir_name(dir) {
                continue;
            }
        }
        out.push(entry);
    }
    Ok(out)
}

// 读缓存：命中返回索引原文 / read the cache: the verbatim body on a hit
async fn read_index_cache(pool: &sqlx::SqlitePool) -> Result<Option<String>, String> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT value FROM settings WHERE key = ?")
            .bind(STORE_CACHE_KEY)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("StorageError: {e}"))?;
    Ok(row.map(|(v,)| v))
}

// 写缓存：索引原文原样落库（解析按次进行，缓存格式与源一致）
// write the cache: the body verbatim (parsing stays per-use; the cache mirrors the source)
async fn write_index_cache(pool: &sqlx::SqlitePool, body: &str) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, CURRENT_TIMESTAMP)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP",
    )
    .bind(STORE_CACHE_KEY)
    .bind(body)
    .execute(pool)
    .await
    .map_err(|e| format!("StorageError: {e}"))?;
    Ok(())
}

// 市场列表：refresh=false 读缓存（无缓存则回源）；refresh=true 强制回源并更新缓存
// Store listing: refresh=false serves the cache (fetching only when empty);
// refresh=true forces a refetch and updates the cache
#[tauri::command]
pub async fn store_list(app: AppHandle, refresh: bool) -> Result<Value, String> {
    let pool = crate::plugin::pool::sqlite_pool(&app).await?;

    if !refresh {
        if let Some(cached) = read_index_cache(&pool).await? {
            let entries = parse_store_index(&cached)?;
            return Ok(serde_json::to_value(entries).map_err(|e| e.to_string())?);
        }
    }

    let body = store_client()
        .get(STORE_INDEX_URL)
        .send()
        .await
        .map_err(|e| format!("HttpError: {e}"))?
        .error_for_status()
        .map_err(|e| format!("HttpError: {e}"))?
        .text()
        .await
        .map_err(|e| format!("HttpError: {e}"))?;
    let entries = parse_store_index(&body)?;
    write_index_cache(&pool, &body).await?;
    Ok(serde_json::to_value(entries).map_err(|e| e.to_string())?)
}

// 下载 + 防护提取 + 安装/更新：zipball → 目标子树（仓库根或 <根>/<dir>/）→ 临时目录
// → install_from_dir
// Download + guarded extraction + install/update: zipball -> the target subtree (the
// repo root or <root>/<dir>/) -> temp dir -> install_from_dir
#[tauri::command]
pub async fn store_install(
    app: AppHandle,
    repo: String,
    dir: Option<String>,
) -> Result<Value, String> {
    if !is_valid_repo(&repo) {
        return Err(format!("InvalidRepo: {repo}"));
    }
    // dir 缺省 = 插件仓库根即插件包 / a missing dir means the repo root is the package
    let dir = dir.unwrap_or_default();
    if !dir.is_empty() && !is_valid_dir_name(&dir) {
        return Err(format!("InvalidDir: {dir}"));
    }

    let bytes = store_client()
        .get(format!("https://api.github.com/repos/{repo}/zipball/HEAD"))
        .send()
        .await
        .map_err(|e| format!("HttpError: {e}"))?
        .error_for_status()
        .map_err(|e| format!("HttpError: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("HttpError: {e}"))?;
    if bytes.len() > MAX_DOWNLOAD_BYTES {
        return Err("HttpError: zipball exceeds the download cap".into());
    }

    // 临时目录：进程 id + 纳秒时钟（无随机源依赖）
    // Temp dir: pid + nanosecond clock (no RNG dependency)
    let tmp = std::env::temp_dir().join(format!(
        "kapi-store-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let result = extract_plugin_subtree(bytes.to_vec(), &dir, &tmp).await?;
    let installed = install_from_dir(&app, &result, true).await;
    let _ = std::fs::remove_dir_all(&tmp);
    installed
}

// zip 条目名安全：'/' 分隔、无空段 / ".."、非绝对路径、无反斜杠
// Zip entry-name safety: '/'-separated, no empty/.. segments, not absolute, no backslash
fn safe_zip_segments(name: &str) -> Option<Vec<&str>> {
    if name.contains('\\') || name.starts_with('/') {
        return None;
    }
    let trimmed = name.strip_prefix("./").unwrap_or(name);
    let segs: Vec<&str> = trimmed.split('/').collect();
    if segs.iter().any(|s| s.is_empty() || *s == ".." || *s == ".") {
        // 末尾空段是目录条目（"a/b/"）的唯一合法形态
        // A trailing empty segment is the only legal form (directory entries, "a/b/")
        let n = segs.len();
        let dir_entry = n >= 2 && segs[n - 1].is_empty() && segs[..n - 1].iter().all(|s| !s.is_empty() && *s != ".." && *s != ".");
        if !dir_entry {
            return None;
        }
        return Some(segs[..n - 1].to_vec());
    }
    Some(segs)
}

// 防护提取：仅解压 <zip 根前缀>/<dir>/ 子树（剥离前缀落到 dest），带条目数 / 总量 /
// 符号链接三重上限；返回解压出的插件目录
// Guarded extraction: unpack only the <zip root prefix>/<dir>/ subtree (prefix stripped
// into dest) under the entry-count / total-size / symlink caps; returns the plugin dir
async fn extract_plugin_subtree(
    zip_bytes: Vec<u8>,
    dir: &str,
    dest: &Path,
) -> Result<PathBuf, String> {
    // 解压放阻塞线程（CPU/IO 密集，不占异步 worker）
    // Extraction runs on a blocking thread (CPU/IO-heavy, off the async workers)
    let dir = dir.to_string();
    let dest = dest.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))
            .map_err(|e| format!("HttpError: invalid zipball ({e})"))?;
        if archive.len() > MAX_ZIP_ENTRIES {
            return Err(format!(
                "HttpError: zipball exceeds the entry cap ({MAX_ZIP_ENTRIES})"
            ));
        }

        // zip 根前缀：GitHub zipball 的所有条目都位于 <repo>-<ref>/ 下
        // Zip root prefix: every GitHub zipball entry sits under <repo>-<ref>/
        let root_prefix = {
            let first = archive
                .by_index(0)
                .map_err(|e| format!("HttpError: invalid zipball ({e})"))?;
            let name = first.name().to_string();
            drop(first);
            match name.split('/').next() {
                Some(p) if !p.is_empty() => format!("{p}/"),
                _ => return Err("HttpError: zipball has no root prefix".into()),
            }
        };
        // 子树前缀：dir 空 = 仓库根即插件包（独立插件仓库），否则 <根>/<dir>/
        // Subtree prefix: an empty dir means the repo root is the plugin package (an
        // independent plugin repo); otherwise <root>/<dir>/
        let subtree = if dir.is_empty() {
            root_prefix.clone()
        } else {
            format!("{root_prefix}{dir}/")
        };

        let mut total: u64 = 0;
        let mut matched = false;
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("HttpError: invalid zipball ({e})"))?;
            let name = entry.name().to_string();
            // 全名先做一次安全校验（前缀匹配前拦截恶意构造）
            // Validate the full name first (hostile shapes die before prefix matching)
            if safe_zip_segments(&name).is_none() {
                return Err(format!("HttpError: unsafe zip entry name: {name}"));
            }
            // 符号链接条目直接拒绝（市场源只允许普通文件）
            // Symlink entries are rejected outright (the store allows regular files only)
            if let Some(mode) = entry.unix_mode() {
                if mode & 0o170000 == 0o120000 {
                    return Err(format!("HttpError: symlink zip entry: {name}"));
                }
            }
            let Some(rel) = name.strip_prefix(&subtree) else {
                continue;
            };
            matched = true;
            if rel.is_empty() {
                continue; // 子树根目录条目 / the subtree root entry itself
            }
            let Some(rel_segs) = safe_zip_segments(rel) else {
                return Err(format!("HttpError: unsafe zip entry name: {name}"));
            };

            total += entry.size();
            if total > MAX_ZIP_TOTAL_BYTES {
                return Err(format!(
                    "HttpError: zipball exceeds the size cap ({} MiB)",
                    MAX_ZIP_TOTAL_BYTES / (1024 * 1024)
                ));
            }

            let mut target = dest.clone();
            for seg in &rel_segs {
                target.push(seg);
            }
            if entry.is_dir() {
                std::fs::create_dir_all(&target)
                    .map_err(|e| format!("StorageError: extract failed ({e})"))?;
                continue;
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("StorageError: extract failed ({e})"))?;
            }
            let mut buf = Vec::with_capacity(entry.size() as usize);
            entry
                .read_to_end(&mut buf)
                .map_err(|e| format!("HttpError: zip read failed ({e})"))?;
            // 二次核验实际解压量（size() 只是声明值）
            // Double-check the actually-read size (size() is only the declared value)
            if buf.len() as u64 > MAX_ZIP_TOTAL_BYTES {
                return Err("HttpError: zip entry exceeds the size cap".into());
            }
            std::fs::write(&target, &buf)
                .map_err(|e| format!("StorageError: extract failed ({e})"))?;
        }

        if !matched {
            return Err(format!("UnknownPlugin: no '{dir}' directory in the zipball"));
        }
        Ok(dest)
    })
    .await
    .map_err(|e| format!("StorageError: extraction task failed ({e})"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- 源标识校验 / source identifier validation ----

    #[test]
    fn validates_repo_and_dir_names() {
        assert!(is_valid_repo("kapi-software/kapi-plugins"));
        assert!(is_valid_repo("a.b_c/d-e.f"));
        assert!(!is_valid_repo("../x"));
        assert!(!is_valid_repo("owner"));
        assert!(!is_valid_repo("owner/repo/extra"));
        assert!(!is_valid_repo("owner/"));
        assert!(!is_valid_repo("own er/x"));
        assert!(is_valid_dir_name("plugin-a"));
        assert!(!is_valid_dir_name("../evil"));
        assert!(!is_valid_dir_name(""));
    }

    // ---- 索引解析 / index parsing ----

    #[test]
    fn parses_index_entries_with_optional_dir() {
        let body = r#"{
            "plugins": [
                { "id": "com.kapi.clipboard", "repo": "kapi-software/kapi-plugins-clipboard" },
                { "id": "com.a", "name": "A", "version": "1.0.0", "repo": "kapi-software/kapi-plugins", "dir": "pluginA" }
            ]
        }"#;
        let entries = parse_store_index(body).unwrap();
        assert_eq!(entries.len(), 2);
        // 独立仓库：dir 缺省 = 仓库根即插件包 / independent repo: no dir means the root
        assert_eq!(entries[0].repo, "kapi-software/kapi-plugins-clipboard");
        assert!(entries[0].dir.is_none());
        // 索引仓库子目录形态 / the in-repo subdir shape
        assert_eq!(entries[1].dir.as_deref(), Some("pluginA"));
    }

    #[test]
    fn index_parsing_drops_invalid_entries_but_keeps_the_rest() {
        let body = r#"{
            "plugins": [
                { "id": "", "repo": "a/b" },
                { "id": "com.no-repo" },
                { "id": "com.bad-repo", "repo": "../evil" },
                { "id": "com.bad-dir", "repo": "a/b", "dir": "../up" },
                { "id": "com.good", "repo": "a/b", "dir": "pkg" }
            ]
        }"#;
        let entries = parse_store_index(body).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "com.good");
    }

    #[test]
    fn index_parsing_rejects_structural_errors() {
        assert!(parse_store_index("{ not json").is_err());
        assert!(parse_store_index(r#"{"no_plugins": []}"#).is_err());
    }

    // ---- zip 条目名安全 / zip entry-name safety ----

    #[test]
    fn safe_zip_segments_accepts_normal_names() {
        assert_eq!(
            safe_zip_segments("pkg-abc/plugin/web/index.html"),
            Some(vec!["pkg-abc", "plugin", "web", "index.html"])
        );
        // 目录条目（尾斜杠）合法且丢弃空尾段 / directory entries are legal (trailing slash dropped)
        assert_eq!(
            safe_zip_segments("pkg-abc/plugin/web/"),
            Some(vec!["pkg-abc", "plugin", "web"])
        );
    }

    #[test]
    fn safe_zip_segments_rejects_traversal_and_abs() {
        assert_eq!(safe_zip_segments("../evil"), None);
        assert_eq!(safe_zip_segments("a/../../b"), None);
        assert_eq!(safe_zip_segments("/abs"), None);
        assert_eq!(safe_zip_segments("a\\b"), None);
        assert_eq!(safe_zip_segments("a//b"), None);
    }

    // ---- 防护提取 / guarded extraction ----

    // 构造内存 zip（Stored 方法：写侧无需压缩特性）
    // Build an in-memory zip (Stored method: no compression feature needed for writing)
    fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default();
        for (name, data) in entries {
            if name.ends_with('/') {
                w.add_directory(name.trim_end_matches('/'), opts).unwrap();
            } else {
                w.start_file(*name, opts).unwrap();
                std::io::Write::write_all(&mut w, data).unwrap();
            }
        }
        w.finish().unwrap().into_inner()
    }

    fn temp_dest(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("kapi-store-test-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn extracts_only_the_plugin_subtree() {
        let zip = build_zip(&[
            ("repo-abc/README.md", b"root readme"),
            ("repo-abc/plugin-a/manifest.json", b"{\"id\":\"com.a\"}"),
            ("repo-abc/plugin-a/web/index.html", b"<html>a</html>"),
            ("repo-abc/plugin-b/manifest.json", b"{\"id\":\"com.b\"}"),
        ]);
        let dest = temp_dest("subtree");
        let out = extract_plugin_subtree(zip, "plugin-a", &dest).await.unwrap();
        assert!(out.join("manifest.json").is_file());
        assert!(out.join("web/index.html").is_file());
        // 其它插件目录不落盘 / sibling plugin dirs never land on disk
        assert!(!out.join("README.md").exists());
        let _ = std::fs::remove_dir_all(&dest);
    }

    #[tokio::test]
    async fn extracts_repo_root_when_dir_empty() {
        // 独立插件仓库：zipball 根即插件包（manifest 在仓库根，README/LICENSE 一并落盘无害）
        // An independent plugin repo: the zipball root is the package (manifest at the
        // repo root; README/LICENSE landing alongside is harmless)
        let zip = build_zip(&[
            ("repo-clip/manifest.json", b"{\"id\":\"com.kapi.clipboard\"}"),
            ("repo-clip/web/index.html", b"<html>clip</html>"),
            ("repo-clip/README.md", b"readme"),
        ]);
        let dest = temp_dest("root");
        let out = extract_plugin_subtree(zip, "", &dest).await.unwrap();
        assert!(out.join("manifest.json").is_file());
        assert!(out.join("web/index.html").is_file());
        assert!(out.join("README.md").is_file());
        let _ = std::fs::remove_dir_all(&dest);
    }

    #[tokio::test]
    async fn rejects_missing_subtree() {
        let zip = build_zip(&[("repo-abc/other/manifest.json", b"{}")]);
        let dest = temp_dest("missing");
        assert!(extract_plugin_subtree(zip, "plugin-a", &dest).await.is_err());
        let _ = std::fs::remove_dir_all(&dest);
    }
}
