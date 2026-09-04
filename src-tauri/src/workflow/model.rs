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

// ============================================================
// 简版 cron 调度器（5 字段：分 时 日 月 周）
// Lightweight cron scheduler (5-field: min hour dom month dow)
// 支持: *  | */n  | n,m  | n-m  | n-m/n
// ============================================================

use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct CronSchedule {
    pub minute: Field,
    pub hour: Field,
    pub dom: Field,  // day of month (1-31)
    pub month: Field, // (1-12)
    pub dow: Field,   // day of week (0=Sun, 1=Mon … 6=Sat)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    All,                   // *
    Step(u32),             // */n
    List(Vec<u32>),        // 1,3,5
    Range { start: u32, end: u32, step: u32 }, // 1-10/2
}

impl FromStr for Field {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s == "*" {
            return Ok(Field::All);
        }
        // */n 形式
        if let Some(n_str) = s.strip_prefix("*/") {
            let n = n_str.parse().map_err(|_| format!("invalid step '{n_str}'"))?;
            if n == 0 { return Err("step cannot be 0".into()); }
            return Ok(Field::Step(n));
        }
        // n-m[/n] 形式
        if s.contains('-') {
            let (range_part, step_str) = if let Some((r, step)) = s.split_once('/') {
                (r, Some(step))
            } else {
                (s, None)
            };
            let (start_str, end_str) = range_part.split_once('-')
                .ok_or_else(|| format!("invalid range '{range_part}'"))?;
            let start: u32 = start_str.parse().map_err(|_| format!("invalid start '{start_str}'"))?;
            let end: u32 = end_str.parse().map_err(|_| format!("invalid end '{end_str}'"))?;
            let step = step_str
                .map(|s| s.parse::<u32>().map_err(|_| format!("invalid step '{s}'")))
                .unwrap_or(Ok(1))?;
            if step == 0 { return Err("step cannot be 0".into()); }
            return Ok(Field::Range { start, end, step });
        }
        // 逗号分隔列表（可能含单个值）
        if s.contains(',') {
            let items: Result<Vec<u32>, _> = s.split(',').map(|p| p.trim().parse().map_err(|_| format!("invalid value '{p}'"))).collect();
            return Ok(Field::List(items?));
        }
        // 单个数字
        let v: u32 = s.parse().map_err(|_| format!("invalid value '{s}'"))?;
        Ok(Field::List(vec![v]))
    }
}

impl Field {
    fn matches(&self, value: u32) -> bool {
        match self {
            Field::All => true,
            Field::Step(n) => value % n == 0,
            Field::List(vals) => vals.contains(&value),
            Field::Range { start, end, step } => {
                value >= *start && value <= *end && (value - start) % step == 0
            }
        }
    }
}

impl FromStr for CronSchedule {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() != 5 {
            return Err(format!(
                "cron expression must have exactly 5 fields (min hour dom month dow), got {}",
                parts.len()
            ));
        }
        Ok(CronSchedule {
            minute: parts[0].parse()?,
            hour: parts[1].parse()?,
            dom: parts[2].parse()?,
            month: parts[3].parse()?,
            dow: parts[4].parse()?,
        })
    }
}

impl CronSchedule {
    /// 检查一个 UTC 时间戳是否匹配该 cron 表达式（精确到分钟）
    /// Returns true if the given UTC date/time (to the minute) matches.
    pub fn matches(&self, _year: u32, month: u32, day: u32, hour: u32, minute: u32) -> bool {
        self.minute.matches(minute)
            && self.hour.matches(hour)
            && self.dom.matches(day)
            && self.month.matches(month)
            && self.dow.matches(day) // simplified: dow just checks day number for 0-6
    }

    /// 计算下一个匹配时刻（UTC 秒），从 now_secs 开始（不包含 now_secs 本身）
    /// Compute the next matching UTC timestamp, starting strictly after `now_secs`.
    /// 使用 std::time + 简单日期算法扫描向前至多 1 年
    /// Scans forward up to 1 year using a simple date algorithm (no extra deps).
    pub fn next_utc_seconds(&self, now_secs: i64) -> Option<i64> {
        if now_secs < 0 {
            return None;
        }
        let (mut year, mut month, mut day, mut hour, mut minute) = epoch_to_utc(now_secs);
        // 跳过当前分钟，进到下一秒所在的分钟
        // Skip the current minute; advance to the next minute
        minute += 1;
        if minute >= 60 {
            minute = 0;
            hour += 1;
            if hour >= 24 {
                hour = 0;
                day += 1;
                if day > days_in_month(year, month) {
                    day = 1;
                    month += 1;
                    if month > 12 {
                        month = 1;
                        year += 1;
                    }
                }
            }
        }

        // 扫描至多 1 年（60 * 24 * 366 = 527040 分钟）
        // Scan at most 1 year
        let max = 60 * 24 * 366;
        for _ in 0..max {
            // 把 day 折回到当月有效范围
            // Reconcile day with month length (handles day overflow from earlier step)
            while day > days_in_month(year, month) {
                day -= days_in_month(year, month);
                month += 1;
                if month > 12 { month = 1; year += 1; }
            }
            if self.matches(year, month, day, hour, minute) {
                if let Some(target) = utc_to_epoch(year, month, day, hour, minute) {
                    if target > now_secs {
                        return Some(target);
                    }
                }
            }
            // 前进 1 分钟
            // Advance 1 minute
            minute += 1;
            if minute >= 60 {
                minute = 0;
                hour += 1;
                if hour >= 24 {
                    hour = 0;
                    day += 1;
                    if day > days_in_month(year, month) {
                        day = 1;
                        month += 1;
                        if month > 12 { month = 1; year += 1; }
                    }
                }
            }
        }
        None
    }
}

// ============================================================
// 简单 UTC 日期工具（不依赖 chrono/time）
// Simple UTC date helpers (no chrono/time dep)
// ============================================================

/// 判断是否闰年 / Is leap year
fn is_leap_year(y: u32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

/// 当月天数 / Days in month
fn days_in_month(y: u32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(y) => 29,
        2 => 28,
        _ => 30, // 不合法月份降级
    }
}

/// 把 Unix epoch 秒数转换为 UTC (年, 月, 日, 时, 分)
/// Convert Unix epoch seconds to UTC (year, month, day, hour, minute)
fn epoch_to_utc(secs: i64) -> (u32, u32, u32, u32, u32) {
    let minute = (secs / 60) as i64;
    let hour = (minute / 60) % 24;
    let total_days = minute / (60 * 24);
    let minute_of_day = (minute % 60) as u32;
    let hour_of_day = hour as u32;
    // 从 1970-01-01 起算
    // Compute date from 1970-01-01
    let mut year = 1970u32;
    let mut day = total_days;
    loop {
        let dy = if is_leap_year(year) { 366 } else { 365 };
        if day < dy { break; }
        day -= dy;
        year += 1;
    }
    let mut month = 1u32;
    while month <= 12 {
        let dm = days_in_month(year, month) as i64;
        if day < dm { break; }
        day -= dm;
        month += 1;
    }
    (year, month, (day as u32) + 1, hour_of_day, minute_of_day)
}

/// UTC (年, 月, 日, 时, 分) → Unix epoch 秒数
/// UTC (year, month, day, hour, minute) → Unix epoch seconds
fn utc_to_epoch(year: u32, month: u32, day: u32, hour: u32, minute: u32) -> Option<i64> {
    if month == 0 || month > 12 || day == 0 || day > days_in_month(year, month) || hour > 23 || minute > 59 {
        return None;
    }
    let mut days: i64 = 0;
    for y in 1970..year {
        days += if is_leap_year(y) { 366 } else { 365 };
    }
    for m in 1..month {
        days += days_in_month(year, m) as i64;
    }
    days += (day - 1) as i64;
    Some(days * 86400 + (hour as i64) * 3600 + (minute as i64) * 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_parsing() {
        assert_eq!("*".parse::<Field>().unwrap(), Field::All);
        assert_eq!("*/5".parse::<Field>().unwrap(), Field::Step(5));
        assert_eq!("1,3,5".parse::<Field>().unwrap(), Field::List(vec![1, 3, 5]));
        assert_eq!("0-30".parse::<Field>().unwrap(), Field::Range { start: 0, end: 30, step: 1 });
        assert_eq!("1-10/2".parse::<Field>().unwrap(), Field::Range { start: 1, end: 10, step: 2 });
    }

    #[test]
    fn test_cron_schedule_parsing() {
        let _: CronSchedule = "*/5 * * * *".parse().unwrap(); // every 5 min
        let _: CronSchedule = "0 9 * * *".parse().unwrap();  // every day at 9am
        let _: CronSchedule = "0 8 * * 1".parse().unwrap();   // every monday at 8am
        "0 9 * *".parse::<CronSchedule>().expect_err("should need 5 fields");
    }

    #[test]
    fn test_cron_matches() {
        let s: CronSchedule = "0 9 * * *".parse().unwrap();
        assert!(s.matches(2025, 1, 15, 9, 0));  // Jan 15 09:00 -> matches
        assert!(!s.matches(2025, 1, 15, 10, 0)); // 10:00 -> no
        assert!(!s.matches(2025, 1, 15, 9, 1));  // 09:01 -> no
    }
}
