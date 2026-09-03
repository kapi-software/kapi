// 工作流数据模型：触发器 / DAG 节点与边 / 执行记录
// Workflow data model: triggers, DAG nodes & edges, execution records
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
