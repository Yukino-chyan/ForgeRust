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
    pub explanation: String, // AI生成的题目解析，导入时预生成  
}  
  
// 2. 导入文件中单道题的格式
#[derive(Debug, Deserialize)]  
pub struct ImportQuestion {  
    pub question_type: String,  
    pub content: String,  
    pub options: Option<Vec<String>>, // 导入时是数组，入库前转成 JSON 字符串  
    pub tags: String,  
    pub difficulty: i32,  
    // 这两个字段导入时可能为空，留给 AI 补全  
    #[serde(default)]  
    pub standard_answer: Option<String>,  
    #[serde(default)]  
    pub explanation: Option<String>,  
}  
  
// 3. 答题评分返回结构  
#[derive(Debug, Serialize, Deserialize)]  
pub struct EvaluateResponse {  
    pub standard_answer: String, // 从 DB 直接取，始终有值  
    pub explanation: String,     // 题目解析，从 DB 直接取  
    pub is_correct: Option<bool>,// 选择题专用：是否答对；简答题为 None  
    pub ai_comment: String,      // AI 针对用户作答的实时点评  
    pub score: i32,              // 0-100  
}  
  
// 4. 导入结果通知
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