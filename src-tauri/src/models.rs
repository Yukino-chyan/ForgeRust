use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// 1. 题库模型
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Question {
    pub id: i32,
    pub content: String,
    pub tags: String, 
    pub difficulty: i32,
}

// 2. AI 响应模型（Mock 阶段的标准格式）
#[derive(Debug, Serialize, Deserialize)]
pub struct MockAiResponse {
    pub score: i32,
    pub comment: String,
    pub next_topic_suggestion: String,
}