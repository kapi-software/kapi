-- ============================================================
-- 003_wal.sql  启用 WAL 日志模式（修复并发写 "database is locked"）
-- Enable WAL journal mode (fixes concurrent-write "database is locked")
-- 背景：Rust 桥接（宿主导入写日志 / headless 落库 / wasm invoke 上下文查询）
-- 与前端（日志页轮询、设置写入、插件列表）在同一 SQLite 文件上并发访问；
-- 默认 DELETE 日志模式下写锁排斥读锁，超时即报 code 5。
-- Background: the Rust bridge (host log writes / headless logging / wasm
-- context queries) and the frontend (log polling, settings writes, plugin
-- lists) share one SQLite file; the default DELETE journal lets a writer
-- block all readers, surfacing as busy code 5 after the timeout.
-- WAL 允许读写并行（journal_mode 持久存于库文件，对新连接同样生效）；
-- synchronous=NORMAL 为 WAL 的推荐搭配（仅断电可能丢最后一次 checkpoint，
-- 不损坏库文件）。
-- WAL lets reads and writes proceed concurrently (the mode persists in the
-- database file, so new connections inherit it); synchronous=NORMAL is the
-- recommended WAL pairing (a power loss may drop the last checkpoint only,
-- never corrupting the file).
-- ============================================================

PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
