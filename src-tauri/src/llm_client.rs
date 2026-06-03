use reqwest::Client;
use serde_json::{json, Value};

pub(crate) enum SseEvent {
    Delta(String),
    Done,
    Other,
}

// 解析一行 SSE。OpenAI 兼容流式格式：每行形如 `data: {json}`，结束行 `data: [DONE]`。
pub(crate) fn parse_sse_line(line: &str) -> SseEvent {
    let line = line.trim();
    if !line.starts_with("data:") {
        return SseEvent::Other;
    }
    let data = line["data:".len()..].trim();
    if data == "[DONE]" {
        return SseEvent::Done;
    }
    match serde_json::from_str::<Value>(data) {
        Ok(v) => {
            let delta = v["choices"][0]["delta"]["content"].as_str().unwrap_or("");
            if delta.is_empty() {
                SseEvent::Other
            } else {
                SseEvent::Delta(delta.to_string())
            }
        }
        Err(_) => SseEvent::Other,
    }
}

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
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    temperature: f64,
    max_tokens: u32,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("API Key 未配置，请点击左下角「设置」填写。".into());
    }

    let client = Client::new();
    let request_body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user",   "content": user_prompt   }
        ],
        "temperature": temperature,
        "max_tokens": max_tokens
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

pub async fn generate_answer_and_explanation_with_tags(
    api_url: &str,
    api_key: &str,
    model: &str,
    allowed_tags: &[String],
    question_type: &str,
    content: &str,
    options: Option<&str>,
) -> Result<(String, String, String), String> {
    let fallback_tags = vec!["Java".to_string(), "Rust".to_string(), "其他".to_string()];
    let tags = if allowed_tags.is_empty() {
        &fallback_tags
    } else {
        allowed_tags
    };
    let tags_context = tags.join(", ");
    let system_prompt = format!(
        "你是 IT 面试题解析助手。请根据给定题目生成标准答案、解析，并从候选考点中选择最合适的一个。候选考点只能是：[{}]。只返回 JSON：{{\"standard_answer\":\"...\",\"explanation\":\"...\",\"tag\":\"...\"}}",
        tags_context
    );
    let user_prompt = format!(
        "题型：{}\n题目：{}\n选项：{}",
        question_type,
        content,
        options.unwrap_or("")
    );

    let raw = call_api(api_url, api_key, model, &system_prompt, &user_prompt, 0.3, 1024).await?;
    let parsed: Value = serde_json::from_str(clean_json(&raw))
        .map_err(|e| format!("JSON 解析失败: {}，原始内容: {}", e, raw))?;

    let ans = parsed["standard_answer"].as_str().unwrap_or("").to_string();
    let exp = parsed["explanation"].as_str().unwrap_or("").to_string();
    let tag = parsed["tag"].as_str().unwrap_or("其他").to_string();

    Ok((ans, exp, tag))
}

#[allow(dead_code, unused_variables)]
pub async fn generate_answer_and_explanation(
    api_url: &str,
    api_key: &str,
    model: &str,
    allowed_tags: &[String],
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

    let raw = call_api(api_url, api_key, model, &system_prompt, &user_prompt, 0.3, 1024).await?;

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
    model: &str,
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

    let raw = call_api(api_url, api_key, model, system_prompt, &user_prompt, 0.3, 1024).await?;

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

pub async fn generate_single_question(
    api_url: &str,
    api_key: &str,
    model: &str,
    topic: &str,
    question_type: &str,
    difficulty: i32,
    requirement: Option<&str>,
) -> Result<crate::models::GeneratedQuestion, String> {
    let (type_desc, format_hint) = match question_type {
        "SINGLE" => (
            "单选题",
            r#"{"content":"题目内容（不含选项）","options":["A. ...","B. ...","C. ...","D. ..."],"standard_answer":"A","explanation":"解析..."}"#,
        ),
        "MULTI" => (
            "多选题",
            r#"{"content":"题目内容（不含选项）","options":["A. ...","B. ...","C. ...","D. ..."],"standard_answer":"AB","explanation":"解析..."}"#,
        ),
        _ => (
            "简答题",
            r#"{"content":"题目内容","options":null,"standard_answer":"参考答案...","explanation":"补充说明..."}"#,
        ),
    };

    let req = requirement
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let requirement_block_system = match req {
        Some(r) => format!(
            "\n【用户具体要求】（必须严格遵守）：{}\n出题时必须紧扣该要求；若该要求与考点方向冲突，以用户要求为准。",
            r
        ),
        None => String::new(),
    };
    let requirement_block_user = match req {
        Some(r) => format!("\n具体要求：{}", r),
        None => String::new(),
    };

    let system_prompt = format!(
        "你是一个专业的IT技术面试题出题专家。\
        请生成一道关于【{topic}】的{type_desc}，难度等级 {difficulty}/5（1最简单，5最难）。{req_sys}\
        必须严格按照以下 JSON 格式输出，不含任何 Markdown 标记或多余文字：\n{format_hint}",
        topic = topic,
        type_desc = type_desc,
        difficulty = difficulty,
        req_sys = requirement_block_system,
        format_hint = format_hint,
    );

    let user_prompt = format!(
        "请生成一道关于【{topic}】的{type_desc}，难度 {difficulty}/5。{req_user}\n直接输出 JSON，不要解释。",
        topic = topic,
        type_desc = type_desc,
        difficulty = difficulty,
        req_user = requirement_block_user,
    );

    let raw = call_api(api_url, api_key, model, &system_prompt, &user_prompt, 0.75, 2048).await?;
    let parsed: Value = serde_json::from_str(clean_json(&raw))
        .map_err(|e| format!("AI 返回格式异常: {}，原始: {}", e, raw))?;

    let content = parsed["content"].as_str().unwrap_or("").trim().to_string();
    if content.is_empty() {
        return Err(format!("AI 返回的题目内容为空，原始: {}", raw));
    }

    let options: Option<Vec<String>> = parsed["options"].as_array().map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    });

    let standard_answer = parsed["standard_answer"].as_str().unwrap_or("").to_string();
    let explanation = parsed["explanation"].as_str().unwrap_or("").to_string();

    Ok(crate::models::GeneratedQuestion {
        question_type: question_type.to_string(),
        content,
        options,
        standard_answer,
        explanation,
        tags: topic.to_string(),
        difficulty,
    })
}

pub async fn evaluate_mock_interview_answer(
    api_url: &str,
    api_key: &str,
    model: &str,
    question: &str,
    standard_answer: &str,
    user_answer: &str,
) -> Result<(i32, String, String), String> {
    let system_prompt = concat!(
        "你是严谨但友好的技术面试官。请评价候选人的回答，给出 0-100 分，",
        "指出关键问题，并提出一个简短追问。只返回 JSON：",
        r#"{"score":80,"comment":"...","follow_up":"..."}"#
    );
    let user_prompt = format!(
        "面试题：{}\n\n参考答案：{}\n\n候选人回答：{}",
        question, standard_answer, user_answer
    );
    let raw = call_api(api_url, api_key, model, system_prompt, &user_prompt, 0.35, 1024).await?;
    let parsed: Value = serde_json::from_str(clean_json(&raw))
        .map_err(|e| format!("AI 面试评价 JSON 解析失败: {}，原始内容: {}", e, raw))?;

    let score = parsed["score"].as_i64().unwrap_or(0).clamp(0, 100) as i32;
    let comment = parsed["comment"].as_str().unwrap_or("").trim().to_string();
    let follow_up = parsed["follow_up"].as_str().unwrap_or("").trim().to_string();

    Ok((
        score,
        if comment.is_empty() { "AI 未返回点评。".into() } else { comment },
        if follow_up.is_empty() { "请再补充一个你认为最关键的细节。".into() } else { follow_up },
    ))
}

pub async fn summarize_mock_interview(
    api_url: &str,
    api_key: &str,
    model: &str,
    transcript: &str,
) -> Result<String, String> {
    let system_prompt = concat!(
        "你是技术面试复盘助手。请基于面试记录生成简短中文总结，包含整体表现、",
        "薄弱知识点、表达建议和下一步复习建议。控制在 180 字以内。"
    );
    call_api(api_url, api_key, model, system_prompt, transcript, 0.4, 1024).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sse_line_extracts_delta() {
        let line = r#"data: {"choices":[{"delta":{"content":"你好"}}]}"#;
        match parse_sse_line(line) {
            SseEvent::Delta(s) => assert_eq!(s, "你好"),
            _ => panic!("应解析出 Delta"),
        }
    }

    #[test]
    fn parse_sse_line_detects_done() {
        assert!(matches!(parse_sse_line("data: [DONE]"), SseEvent::Done));
    }

    #[test]
    fn parse_sse_line_ignores_blank_and_non_data() {
        assert!(matches!(parse_sse_line(""), SseEvent::Other));
        assert!(matches!(parse_sse_line(": keep-alive"), SseEvent::Other));
        // delta 无 content 字段时也视为 Other
        let line = r#"data: {"choices":[{"delta":{}}]}"#;
        assert!(matches!(parse_sse_line(line), SseEvent::Other));
    }
}
