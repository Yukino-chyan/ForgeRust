use reqwest::Client;  
use serde_json::{json, Value};  
  
const API_URL: &str = "https://zenmux.ai/api/v1/chat/completions";  
const API_KEY: &str = "sk-ai-v1-81ca59e8dcadd9d2038477201bac2f1363a325ca1086b653314d93d410c3d8a9";  
  
// ================================  
// 内部通用请求函数（避免重复代码）  
// ================================  
async fn call_api(system_prompt: &str, user_prompt: &str) -> Result<String, String> {  
    let client = Client::new();  
  
    let request_body = json!({  
        "model": "deepseek/deepseek-chat",  
        "messages": [  
            { "role": "system", "content": system_prompt },  
            { "role": "user",   "content": user_prompt   }  
        ],  
        "temperature": 0.3,  
        "max_tokens": 1024  
    });  
  
    let response = client  
        .post(API_URL)  
        .header("Authorization", format!("Bearer {}", API_KEY))  
        .header("Content-Type", "application/json")  
        .json(&request_body)  
        .send()  
        .await  
        .map_err(|e| format!("网络请求失败: {}", e))?;  
  
    if response.status().is_success() {  
        let res_json: Value = response  
            .json()  
            .await  
            .map_err(|e| format!("解析响应 JSON 失败: {}", e))?;  
  
        res_json["choices"][0]["message"]["content"]  
            .as_str()  
            .map(|s| s.to_string())  
            .ok_or_else(|| "API 返回格式异常，找不到 content 字段".into())  
    } else {  
        let status = response.status();  
        let error_text = response.text().await.unwrap_or_default();  
        Err(format!("API 请求失败，状态码: {}，详情: {}", status, error_text))  
    }  
}  
  
// ================================  
// 函数1：导入时补全答案 + 生成解析  
// 适用于：所有题型（导入时没有 standard_answer 或 explanation 的题目）  
// 返回：(standard_answer, explanation)  
// ================================  
pub async fn generate_answer_and_explanation(  
    question_type: &str,  
    content: &str,  
    options: Option<&str>, // 选择题传入格式化后的选项字符串  
) -> Result<(String, String), String> {  
      
    let options_text = options  
        .map(|o| format!("\n选项：{}", o))  
        .unwrap_or_default();  
  
    let system_prompt = concat!(  
        "你是一个专业的IT技术面试题库维护者。",  
        "请为给定的题目生成标准答案和详细解析。",  
        "必须严格按照以下 JSON 格式输出，不要包含任何 Markdown 标记或多余文字：",  
        r#"{"standard_answer": "答案内容", "explanation": "详细解析内容"}"#  
    ); 
  
    let user_prompt = format!(  
        "题目类型：{}\n题目内容：{}{}\n\n请生成标准答案和解析：",  
        question_type, content, options_text  
    );  
  
    let raw = call_api(system_prompt, &user_prompt).await?;  
  
    // 解析 AI 返回的 JSON  
    let parsed: Value = serde_json::from_str(&raw)  
        .map_err(|e| format!("AI 返回内容不是合法 JSON: {}，原始内容: {}", e, raw))?;  
  
    let standard_answer = parsed["standard_answer"]  
        .as_str()  
        .unwrap_or("暂无标准答案")  
        .to_string();  
  
    let explanation = parsed["explanation"]  
        .as_str()  
        .unwrap_or("暂无解析")  
        .to_string();  
  
    Ok((standard_answer, explanation))  
}  
  
// ================================  
// 函数2：答题时对简答题进行实时 AI 点评  
// 适用于：ESSAY 题型，用户提交答案后调用  
// 返回：(score, ai_comment)  
// ================================  
pub async fn evaluate_essay_answer(  
    content: &str,  
    standard_answer: &str,  
    user_answer: &str,  
) -> Result<(i32, String), String> {  
      
    // 修复：修改 Prompt 引导 AI 输出 score 和 comment
    let system_prompt = concat!(  
        "你是一个专业的IT技术面试官。",  
        "请根据标准答案对用户的回答进行评分（0-100）和点评。",  
        "必须严格按照以下 JSON 格式输出，不要包含任何 Markdown 标记或多余文字：",  
        r#"{"score": 评分数字, "comment": "点评内容"}"#  
    ); 

    let user_prompt = format!(  
        "题目：{}\n\n标准答案：{}\n\n用户回答：{}\n\n请评分并点评：",  
        content, standard_answer, user_answer  
    );  
  
    let raw = call_api(system_prompt, &user_prompt).await?;  
  
    let parsed: Value = serde_json::from_str(&raw)  
        .map_err(|e| format!("AI 返回内容不是合法 JSON: {}，原始内容: {}", e, raw))?;  
  
    // 现在字段对应上了
    let score = parsed["score"]  
        .as_i64()  
        .unwrap_or(0) as i32;  // 默认给0，方便发现错误
  
    let comment = parsed["comment"]  
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "AI 返回格式错误，缺少 comment 字段".to_string());  
  
    Ok((score, comment))  
}
