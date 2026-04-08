use serde::{Deserialize, Serialize};
use sqlx::FromRow;

//1.题库模型
#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Question {
    pub id: i32,
    pub question_type: String, // "ESSAY" (简答), "SINGLE" (单选), "MULTI" (多选)
    pub content: String,
    pub options: Option<String>, 
    pub tags: String,
    pub difficulty: i32,
    pub standard_answer: String, // 存储标准答案（用于本地比对或 AI 参考）
}
// 2. AI 响应模型（Mock 阶段的标准格式）
#[derive(Debug, Serialize, Deserialize)]
pub struct MockAiResponse {
    pub score: i32,
    pub comment: String,
    pub next_topic_suggestion: String,
}
