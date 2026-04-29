use reqwest::Client;
use serde_json::{json, Value};

fn clean_json(raw: &str) -> &str {
    raw.trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
}

async fn call_api(
    api_url: &str,
    api_key: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("API Key 未配置，请点击左下角「设置」填写。".into());
    }

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
        .post(api_url)
        .header("Authorization", format!("Bearer {}", api_key))
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

pub async fn generate_answer_and_explanation(
    api_url: &str,
    api_key: &str,
    question_type: &str,
    content: &str,
    options: Option<&str>,
) -> Result<(String, String, String), String> {
    let allowed_tags = vec!["Java", "Rust", "操作系统", "计算机网络", "数据库", "数据结构", "其他"];
    let tags_context = allowed_tags.join(", ");

    let system_prompt = format!(
        "你是一个专业的IT技术面试题库维护者。请为题目生成标准答案、详细解析和最匹配的考点标签。\n\
        【标签约束】：你必须从以下预设标签中选择最合适的一个：[{}]。\n\
        严格按照 JSON 格式输出：\n\
        {{\"standard_answer\": \"...\", \"explanation\": \"...\", \"tag\": \"...\"}}",
        tags_context
    );

    let user_prompt = format!(
        "类型：{}\n内容：{}{}\n请生成：",
        question_type,
        content,
        options.unwrap_or("")
    );

    let raw = call_api(api_url, api_key, &system_prompt, &user_prompt).await?;

    let parsed: Value = serde_json::from_str(clean_json(&raw))
        .map_err(|e| format!("JSON 解析失败: {}，原始内容: {}", e, raw))?;

    let ans = parsed["standard_answer"].as_str().unwrap_or("").to_string();
    let exp = parsed["explanation"].as_str().unwrap_or("").to_string();
    let tag = parsed["tag"].as_str().unwrap_or("其他").to_string();

    Ok((ans, exp, tag))
}

pub async fn evaluate_essay_answer(
    api_url: &str,
    api_key: &str,
    content: &str,
    standard_answer: &str,
    user_answer: &str,
) -> Result<(i32, String), String> {
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

    let raw = call_api(api_url, api_key, system_prompt, &user_prompt).await?;

    let parsed: Value = serde_json::from_str(clean_json(&raw))
        .map_err(|e| format!("AI 返回内容不是合法 JSON: {}，原始内容: {}", e, raw))?;

    let score = parsed["score"]
        .as_i64()
        .ok_or_else(|| format!("AI 返回的 score 字段缺失，原始内容: {}", raw))?
        as i32;

    let comment = parsed["comment"]
        .as_str()
        .ok_or_else(|| format!("AI 返回的 comment 字段缺失，原始内容: {}", raw))?
        .to_string();

    Ok((score, comment))
}
