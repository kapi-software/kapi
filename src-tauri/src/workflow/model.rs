// 工作流数据模型：触发器 / DAG 节点与边 / 执行记录
// Workflow data model: triggers, DAG nodes & edges, execution records
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ============================================================
// 触发器
// Triggers
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TriggerType {
    Clipboard,
    Hotkey,
    Schedule,
    Manual,
    PluginEvent,
}

impl TriggerType {
    pub fn as_str(self) -> &'static str {
        match self {
            TriggerType::Clipboard => "clipboard",
            TriggerType::Hotkey => "hotkey",
            TriggerType::Schedule => "schedule",
            TriggerType::Manual => "manual",
            TriggerType::PluginEvent => "plugin_event",
        }
    }

    #[allow(dead_code)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "clipboard" => Some(Self::Clipboard),
            "hotkey" => Some(Self::Hotkey),
            "schedule" => Some(Self::Schedule),
            "manual" => Some(Self::Manual),
            "plugin_event" => Some(Self::PluginEvent),
            _ => None,
        }
    }
}

/// 触发器条目（内存注册表）
/// Trigger entry (in-memory registry)
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TriggerEntry {
    pub trigger_type: TriggerType,
    pub config: Value,
    pub workflow_id: String,
}

/// 工作流触发器配置（与前端 WorkflowTrigger 对齐）
/// Workflow trigger config (mirrors frontend type)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTrigger {
    pub id: String,
    pub workflow_id: String,
    pub trigger_type: String,
    pub config: Value,
    #[serde(default = "default_true")]
    pub is_enabled: bool,
}

fn default_true() -> bool {
    true
}

// ============================================================
// DAG 数据模型（与前端 src/types/index.ts 对齐）
// DAG data model (mirrors frontend src/types/index.ts)
// ============================================================

/// graph schema 版本号（v0=老数据，v1=当前：含 position 字段 + 标准 bindings 语义）
/// graph schema version (v0=legacy, v1=current: includes position field + standard bindings)
pub const CURRENT_GRAPH_VERSION: i32 = 1;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkflowGraph {
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
    #[serde(default)]
    pub bindings: Vec<DataBinding>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkflowNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkflowEdge {
    pub from: String,
    pub to: String,
    /// 数据映射：上游 output field name → 下游 input field name
    /// Data map: upstream output field name → downstream input field name
    /// 例：{ "text": "content", "meta": "info" } 表示把上游 outputs.text 喂给下游 inputs.content
    /// Example: { "text": "content", "meta": "info" } means upstream outputs.text → downstream inputs.content
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub map: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataBinding {
    pub from: String,
    pub output: String,
    pub to: String,
    pub input: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub graph: WorkflowGraph,
    /// graph schema 版本；缺省视为 0（v0 老数据）
    /// graph schema version; missing treated as 0 (legacy)
    #[serde(rename = "schema_version", default)]
    pub schema_version: i32,
    #[serde(rename = "is_enabled")]
    pub is_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

// ============================================================
// 执行记录
// Execution records
// ============================================================

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowRun {
    pub id: i64,
    pub workflow_id: String,
    pub trigger_type: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowStepLog {
    pub id: i64,
    pub run_id: i64,
    pub step_id: String,
    pub plugin_id: Option<String>,
    pub action: Option<String>,
    pub status: String,
    pub input: Option<String>,
    pub output: Option<String>,
    pub error: Option<String>,
    pub duration_ms: Option<i64>,
    pub created_at: String,
}

// ============================================================
// 图校验
// Graph validation
// ============================================================

/// 校验错误严重程度
/// Validation error severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GraphErrorKind {
    /// 致命：环、悬空边、重复 id——这种图不能运行
    /// Fatal: cycles, dangling edges, duplicate ids — this graph cannot run
    Fatal,
    /// 警告：孤儿节点、重复边等——图能跑但不是好图
    /// Warning: orphan nodes, duplicate edges — graph runs but is suboptimal
    Warning,
}

/// 单条校验错误
/// Single validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphError {
    /// 错误类型（前端用于分类显示 / icon）
    /// Error type (frontend uses this for category display / icon)
    pub kind: GraphErrorKind,
    /// 机器可读代码（duplicate_node_id / dangling_edge / cycle / orphan_node ...）
    /// Machine-readable code
    pub code: String,
    /// 人类可读消息（已国际化 key 或现成字符串；v1 用现成字符串）
    /// Human-readable message
    pub message: String,
    /// 涉及到的 node id（环/孤儿场景；多条边时为 None）
    /// Affected node ids; None for edge-level errors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_ids: Option<Vec<String>>,
    /// 涉及到的边索引（边级错误）
    /// Affected edge indices
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_indices: Option<Vec<usize>>,
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}: {}", self.kind_str(), self.code, self.message)
    }
}

impl GraphError {
    fn kind_str(&self) -> &'static str {
        match self.kind {
            GraphErrorKind::Fatal => "fatal",
            GraphErrorKind::Warning => "warning",
        }
    }

    pub fn fatal(code: &str, message: impl Into<String>) -> Self {
        Self {
            kind: GraphErrorKind::Fatal,
            code: code.to_string(),
            message: message.into(),
            node_ids: None,
            edge_indices: None,
        }
    }

    pub fn warning(code: &str, message: impl Into<String>) -> Self {
        Self {
            kind: GraphErrorKind::Warning,
            code: code.to_string(),
            message: message.into(),
            node_ids: None,
            edge_indices: None,
        }
    }

    pub fn with_nodes(mut self, ids: Vec<String>) -> Self {
        self.node_ids = Some(ids);
        self
    }

    pub fn with_edge_indices(mut self, idx: Vec<usize>) -> Self {
        self.edge_indices = Some(idx);
        self
    }
}

/// 校验结果：有错误就列出来
/// Validation result: empty Vec means valid
pub type ValidationReport = Vec<GraphError>;

/// 当前 graph 是否可运行（无 fatal 错误）
/// Whether the current graph is runnable (no fatal errors)
pub fn is_runnable(report: &ValidationReport) -> bool {
    !report.iter().any(|e| matches!(e.kind, GraphErrorKind::Fatal))
}

// ============================================================
// 内部执行状态
// Internal execution state
// ============================================================

/// 单节点执行结果 / Per-node outcome
#[derive(Debug)]
pub enum NodeOutcome {
    Success { node_id: String, output: Value },
    Failure { node_id: String, error: String },
    Skipped { node_id: String },
}

/// 终止态结果 / Terminal outcome
#[derive(Debug)]
pub struct RunOutcome {
    pub status: &'static str,
    pub error: Option<String>,
}

/// 执行上下文 / Execution context
pub struct WorkflowContext {
    pub trigger: Value,
    pub outputs: std::collections::HashMap<String, Value>,
}
