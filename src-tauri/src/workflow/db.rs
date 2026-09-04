// 工作流 CRUD：workflows / workflow_runs / workflow_step_logs / workflow_triggers
// Workflow CRUD: workflows / workflow_runs / workflow_step_logs / workflow_triggers
use sqlx::{Row, SqlitePool};
use tauri::{AppHandle, Manager};

use crate::workflow::model::{Workflow, WorkflowGraph, WorkflowRun, WorkflowStepLog, WorkflowTrigger};

// ============================================================
// Pool helpers
// ============================================================

/// 直接打开一份 SQLite 池并应用迁移（与 plugin-sql 共用同一 DB 文件）
/// Open a dedicated SQLite pool with migrations applied (same DB file as plugin-sql)
/// setup 阶段 start_triggers 早于前端 Database.load，故此处必须自带迁移；
/// This runs during setup, before the frontend's Database.load, so it must self-migrate
pub async fn open_pool_with_migrations(app: &AppHandle) -> Result<SqlitePool, String> {
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};

    // 与 plugin-sql wrapper.rs 保持一致：app_config_dir + sqlite:kapi.db
    // Match plugin-sql wrapper.rs: app_config_dir + sqlite:kapi.db
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("PathError: {e}"))?;
    let _ = std::fs::create_dir_all(&dir);
    let db_path = dir.join("kapi.db");

    // WAL 经连接选项设置：sqlx 在建立连接（事务外）时执行 PRAGMA，journal_mode
    // 持久化进库文件，前端连接随之受益；003 迁移因此改为无操作占位
    // WAL is set via connect options: sqlx runs the PRAGMA at connection setup
    // (outside transactions); journal_mode persists in the file so frontend
    // connections inherit it — hence 003 became a no-op placeholder
    let opts = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal);

    let pool = SqlitePool::connect_with(opts)
        .await
        .map_err(|e| format!("StorageError: cannot open {} ({e})", db_path.display()))?;
    apply_migrations(&pool).await?;
    Ok(pool)
}

/// 用与 plugin-sql 完全一致的方式跑迁移：共用 _sqlx_migrations 版本表
/// Run migrations exactly like plugin-sql: share the _sqlx_migrations version table
/// 双方都经 SqlxMigration::new 构造（checksum = sha384(sql)），先跑的一方落版本，
/// 后跑的一方校验通过即跳过——避免「表已存在」或迁移被重复执行
/// Both sides build via SqlxMigration::new (checksum = sha384(sql)); whoever runs first
/// records the version, the other validates and skips — no "table already exists", no re-runs
async fn apply_migrations(pool: &SqlitePool) -> Result<(), String> {
    use sqlx::migrate::{Migration as SqlxMigration, Migrator};
    use std::borrow::Cow;

    // 仅保留 Up 迁移（与 plugin-sql MigrationList::resolve 的过滤一致）
    // Keep only Up migrations (matching plugin-sql's MigrationList::resolve filter)
    let migrations: Vec<SqlxMigration> = crate::db::migrations()
        .into_iter()
        .filter(|m| matches!(m.kind, tauri_plugin_sql::MigrationKind::Up))
        .map(|m| {
            // 防御：含 PRAGMA 的迁移标记 no_tx。注意 sqlx-sqlite 的 apply() 实际仍会
            // 无条件包事务（原子记账），PRAGMA 迁移根本无法经 Migrator 落库——WAL 等
            // 连接级 PRAGMA 必须走 SqliteConnectOptions（见 open_pool_with_migrations）
            // Defensive: flag PRAGMA-bearing migrations no_tx. Note sqlx-sqlite's apply()
            // still unconditionally wraps a transaction (atomic bookkeeping), so PRAGMA
            // migrations can never land via the Migrator — connection-level PRAGMAs like
            // WAL belong in SqliteConnectOptions (see open_pool_with_migrations)
            let no_tx = m
                .sql
                .lines()
                .any(|l| l.trim_start().to_uppercase().starts_with("PRAGMA"));
            SqlxMigration::new(
                m.version,
                m.description.into(),
                m.kind.into(),
                m.sql.into(),
                no_tx,
            )
        })
        .collect();

    let migrator = Migrator {
        migrations: Cow::Owned(migrations),
        ignore_missing: false,
        locking: true,
        no_tx: false,
    };
    migrator
        .run(pool)
        .await
        .map_err(|e| format!("MigrationError: {e}"))
}

// ============================================================
// Workflow CRUD
// ============================================================

pub async fn workflow_get(pool: &SqlitePool, workflow_id: &str) -> Result<Option<Workflow>, String> {
    let row = sqlx::query(
        "SELECT id, name, description, graph, is_enabled, created_at, updated_at FROM workflows WHERE id = ?",
    )
    .bind(workflow_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("StorageError: {e}"))?;

    let Some(row) = row else { return Ok(None) };
    let graph_text: String = row.try_get("graph").map_err(|e| format!("StorageError: {e}"))?;
    let graph: WorkflowGraph = serde_json::from_str(&graph_text).map_err(|e| format!("InvalidGraph: {e}"))?;
    Ok(Some(Workflow {
        id: row.try_get("id").map_err(|e| format!("StorageError: {e}"))?,
        name: row.try_get("name").map_err(|e| format!("StorageError: {e}"))?,
        description: row.try_get("description").ok(),
        graph,
        // 老数据无 schema_version 字段时按 0 处理
        // Legacy rows without schema_version are treated as v0
        schema_version: row.try_get("schema_version").unwrap_or(0),
        is_enabled: {
            let v: i64 = row.try_get("is_enabled").map_err(|e| format!("StorageError: {e}"))?;
            v != 0
        },
        created_at: row.try_get("created_at").ok(),
        updated_at: row.try_get("updated_at").ok(),
    }))
}

pub async fn workflow_list(pool: &SqlitePool) -> Result<Vec<Workflow>, String> {
    let rows = sqlx::query(
        "SELECT id, name, description, graph, is_enabled, created_at, updated_at FROM workflows ORDER BY updated_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("StorageError: {e}"))?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let graph_text: String = row.try_get("graph").map_err(|e| format!("StorageError: {e}"))?;
        let graph: WorkflowGraph = serde_json::from_str(&graph_text).map_err(|e| format!("InvalidGraph: {e}"))?;
        out.push(Workflow {
            id: row.try_get("id").map_err(|e| format!("StorageError: {e}"))?,
            name: row.try_get("name").map_err(|e| format!("StorageError: {e}"))?,
            description: row.try_get("description").ok(),
            graph,
            schema_version: row.try_get("schema_version").unwrap_or(0),
            is_enabled: {
                let v: i64 = row.try_get("is_enabled").map_err(|e| format!("StorageError: {e}"))?;
                v != 0
            },
            created_at: row.try_get("created_at").ok(),
            updated_at: row.try_get("updated_at").ok(),
        });
    }
    Ok(out)
}

pub async fn workflow_save(pool: &SqlitePool, workflow: &Workflow) -> Result<(), String> {
    let graph_json = serde_json::to_string(&workflow.graph).map_err(|e| format!("InvalidGraph: {e}"))?;
    sqlx::query(
        "INSERT INTO workflows (id, name, description, graph, is_enabled, updated_at)
         VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
         ON CONFLICT(id) DO UPDATE SET
           name = excluded.name, description = excluded.description, graph = excluded.graph,
           is_enabled = excluded.is_enabled, updated_at = CURRENT_TIMESTAMP",
    )
    .bind(&workflow.id)
    .bind(&workflow.name)
    .bind(workflow.description.as_deref())
    .bind(&graph_json)
    .bind(if workflow.is_enabled { 1 } else { 0 })
    .execute(pool)
    .await
    .map_err(|e| format!("StorageError: {e}"))?;
    Ok(())
}

pub async fn workflow_delete(pool: &SqlitePool, workflow_id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM workflows WHERE id = ?")
        .bind(workflow_id)
        .execute(pool)
        .await
        .map_err(|e| format!("StorageError: {e}"))?;
    Ok(())
}

// ============================================================
// Runs CRUD
// ============================================================

pub async fn workflow_runs(pool: &SqlitePool, workflow_id: &str, limit: i32) -> Result<Vec<WorkflowRun>, String> {
    let rows = sqlx::query(
        "SELECT * FROM workflow_runs WHERE workflow_id = ? ORDER BY started_at DESC LIMIT ?",
    )
    .bind(workflow_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("StorageError: {e}"))?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(WorkflowRun {
            id: row.try_get("id").map_err(|e| format!("StorageError: {e}"))?,
            workflow_id: row.try_get("workflow_id").map_err(|e| format!("StorageError: {e}"))?,
            trigger_type: row.try_get("trigger_type").ok(),
            status: row.try_get("status").map_err(|e| format!("StorageError: {e}"))?,
            error: row.try_get("error").ok(),
            started_at: row.try_get("started_at").map_err(|e| format!("StorageError: {e}"))?,
            finished_at: row.try_get("finished_at").ok(),
        });
    }
    Ok(out)
}

pub async fn workflow_run_steps(pool: &SqlitePool, run_id: i64) -> Result<Vec<WorkflowStepLog>, String> {
    let rows = sqlx::query("SELECT * FROM workflow_step_logs WHERE run_id = ? ORDER BY id")
        .bind(run_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("StorageError: {e}"))?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(WorkflowStepLog {
            id: row.try_get("id").map_err(|e| format!("StorageError: {e}"))?,
            run_id: row.try_get("run_id").map_err(|e| format!("StorageError: {e}"))?,
            step_id: row.try_get("step_id").map_err(|e| format!("StorageError: {e}"))?,
            plugin_id: row.try_get("plugin_id").ok(),
            action: row.try_get("action").ok(),
            status: row.try_get("status").map_err(|e| format!("StorageError: {e}"))?,
            input: row.try_get("input").ok(),
            output: row.try_get("output").ok(),
            error: row.try_get("error").ok(),
            duration_ms: row.try_get("duration_ms").ok(),
            created_at: row.try_get("created_at").map_err(|e| format!("StorageError: {e}"))?,
        });
    }
    Ok(out)
}

// ============================================================
// Trigger CRUD
// ============================================================

pub async fn trigger_save(pool: &SqlitePool, trigger: &WorkflowTrigger) -> Result<(), String> {
    let config_json = serde_json::to_string(&trigger.config).map_err(|e| format!("InvalidConfig: {e}"))?;
    sqlx::query(
        "INSERT INTO workflow_triggers (id, workflow_id, trigger_type, config, is_enabled, updated_at)
         VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
         ON CONFLICT(id) DO UPDATE SET
           workflow_id = excluded.workflow_id, trigger_type = excluded.trigger_type,
           config = excluded.config, is_enabled = excluded.is_enabled, updated_at = CURRENT_TIMESTAMP",
    )
    .bind(&trigger.id)
    .bind(&trigger.workflow_id)
    .bind(&trigger.trigger_type)
    .bind(&config_json)
    .bind(if trigger.is_enabled { 1 } else { 0 })
    .execute(pool)
    .await
    .map_err(|e| format!("StorageError: {e}"))?;
    Ok(())
}

pub async fn trigger_delete(pool: &SqlitePool, trigger_id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM workflow_triggers WHERE id = ?")
        .bind(trigger_id)
        .execute(pool)
        .await
        .map_err(|e| format!("StorageError: {e}"))?;
    Ok(())
}

pub async fn trigger_list(pool: &SqlitePool, workflow_id: Option<&str>) -> Result<Vec<WorkflowTrigger>, String> {
    let rows = if let Some(wid) = workflow_id {
        sqlx::query("SELECT id, workflow_id, trigger_type, config, is_enabled FROM workflow_triggers WHERE workflow_id = ?")
            .bind(wid)
            .fetch_all(pool)
            .await
    } else {
        sqlx::query("SELECT id, workflow_id, trigger_type, config, is_enabled FROM workflow_triggers")
            .fetch_all(pool)
            .await
    }
    .map_err(|e| format!("StorageError: {e}"))?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let config_text: String = row.try_get("config").map_err(|e| format!("StorageError: {e}"))?;
        let config: serde_json::Value = serde_json::from_str(&config_text).map_err(|e| format!("InvalidConfig: {e}"))?;
        let is_enabled: i64 = row.try_get("is_enabled").map_err(|e| format!("StorageError: {e}"))?;
        out.push(WorkflowTrigger {
            id: row.try_get("id").map_err(|e| format!("StorageError: {e}"))?,
            workflow_id: row.try_get("workflow_id").map_err(|e| format!("StorageError: {e}"))?,
            trigger_type: row.try_get("trigger_type").map_err(|e| format!("StorageError: {e}"))?,
            config,
            is_enabled: is_enabled != 0,
        });
    }
    Ok(out)
}
