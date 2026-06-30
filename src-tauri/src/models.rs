use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// 1. 数据库题目模型
#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Question {
    pub id: i32,
    pub question_type: String,
    pub content: String,
    pub options: Option<String>,
    pub tags: String,
    pub difficulty: i32,
    pub standard_answer: String,
    pub explanation: String,
    pub source: String,
    pub quality_status: String,
    pub quality_note: String,
    pub content_hash: String,
    pub duplicate_of: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Topic {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub created_at: String,
}

// 2. 导入文件中单道题的格式
#[derive(Debug, Deserialize)]
pub struct ImportQuestion {
    pub question_type: String,
    pub content: String,
    pub options: Option<Vec<String>>,
    pub tags: String,
    pub difficulty: i32,
    #[serde(default)]
    pub standard_answer: Option<String>,
    #[serde(default)]
    pub explanation: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub quality_status: Option<String>,
    #[serde(default)]
    pub quality_note: Option<String>,
}

// 3. 答题评分返回结构
#[derive(Debug, Serialize, Deserialize)]
pub struct EvaluateResponse {
    pub standard_answer: String,  // 从 DB 直接取，始终有值
    pub explanation: String,      // 题目解析，从 DB 直接取
    pub is_correct: Option<bool>, // 选择题专用：是否答对；简答题为 None
    pub ai_comment: String,       // AI 针对用户作答的实时点评
    pub score: i32,               // 0-100
}

// 4. 保存训练记录（前端传入）
#[derive(Debug, Deserialize)]
pub struct SaveRecordInput {
    pub question_id: i32,
    pub user_answer: String,
    pub score: i32,
    pub is_correct: Option<bool>,
    pub skipped: bool,
    pub manually_added: bool,
    pub time_spent: i32,
}

// 5. 错题本条目（后端查询返回）
#[derive(Debug, Serialize, FromRow)]
pub struct WrongQuestion {
    pub question_id: i32,
    pub content: String,
    pub question_type: String,
    pub tags: String,
    pub difficulty: i32,
    pub standard_answer: String,
    pub explanation: String,
    pub wrong_count: i32,
    pub last_score: i32,
    pub last_attempt: String,
    pub manually_added_count: i32,
}

// 6. 导入结果通知
#[derive(Debug, Serialize, Deserialize)]
pub struct ImportResult {
    pub total: usize,        // 共导入题目数
    pub ai_generated: usize, // AI 补全了几题的答案/解析
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportProgress {
    pub current: usize,
    pub total: usize,
    pub message: String,
    pub is_finished: bool,
}

// 7. AI 出题 - 生成的题目结构
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeneratedQuestion {
    pub question_type: String,
    pub content: String,
    pub options: Option<Vec<String>>,
    pub standard_answer: String,
    pub explanation: String,
    pub tags: String,
    pub difficulty: i32,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub quality_status: String,
    #[serde(default)]
    pub quality_note: String,
}

// 8. AI 出题 - 生成进度事件
#[derive(Debug, Clone, Serialize)]
pub struct GenerateProgress {
    pub current: usize,
    pub total: usize,
    pub question: Option<GeneratedQuestion>,
    pub message: String,
    pub is_finished: bool,
    pub error: Option<String>,
}

// 9. Dashboard 聚合统计
#[derive(Debug, Serialize)]
pub struct DashboardStats {
    pub total_answered: i64,
    pub overall_accuracy: f64, // 0.0 - 1.0
    pub mastered_tags: i64,
    pub total_tags: i64,
    pub pending_review: i64,
    pub streak_days: i64,
    pub today_answered: i64,
    pub week_delta_answered: i64, // 本周 - 上周
    pub week_delta_accuracy: f64, // 百分点
}

// 10. 日趋势点
#[derive(Debug, Serialize)]
pub struct DayPoint {
    pub date: String,  // YYYY-MM-DD
    pub accuracy: f64, // 0.0 - 1.0
    pub count: i64,
}

// 11. 标签掌握度
#[derive(Debug, Serialize)]
pub struct TagStat {
    pub tag: String,
    pub accuracy: f64,
    pub total: i64,
}

// 12. 最近会话记录
#[derive(Debug, Serialize)]
pub struct SessionRecord {
    pub id: i64,
    pub started_at: String,
    pub total: i64,
    pub correct: i64,
    pub tags: Vec<String>,
}

// ── 简历解析 ──
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResumeProject {
    pub name: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub highlights: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ParsedResume {
    #[serde(default)]
    pub candidate: String,
    #[serde(default)]
    pub projects: Vec<ResumeProject>,
    #[serde(default)]
    pub tech_stack: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ResumeRecord {
    pub id: i64,
    pub candidate: String,
    pub projects: Vec<ResumeProject>,
    pub tech_stack: Vec<String>,
}

// ── 对话式面试 ──
#[derive(Debug, Serialize, Clone)]
pub struct InterviewMessage {
    pub role: String,
    pub phase: String,
    pub content: String,
    pub seq: i64,
}

#[derive(Debug, Serialize)]
pub struct InterviewTurn {
    pub message: String,
    pub phase: String,
    pub finished: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InterviewSettings {
    pub target_role: String,
    pub direction: String,
    pub interview_difficulty: String,
    pub follow_up_intensity: String,
    pub practice_mode: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DimensionScores {
    #[serde(default)]
    pub project_depth: i32,
    #[serde(default)]
    pub fundamental_solidity: i32,
    #[serde(default)]
    pub communication: i32,
}

#[derive(Debug, Serialize)]
pub struct InterviewReport2 {
    pub interview_id: i64,
    pub average_score: f64,
    pub dimension_scores: DimensionScores,
    pub summary: String,
    pub weak_points: Vec<String>,
    pub recommended_tags: Vec<String>,
    pub action_items: Vec<String>,
    pub messages: Vec<InterviewMessage>,
}

#[derive(Debug, Serialize)]
pub struct InterviewSummary {
    pub id: i64,
    pub created_at: String,
    pub candidate: String,
    pub tags: String,
    pub average_score: f64,
    pub dimension_scores: DimensionScores,
    pub target_role: String,
    pub direction: String,
}
