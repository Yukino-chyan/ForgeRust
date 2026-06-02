# 阶段二·A 模拟面试后端重构 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把模拟面试后端从「随机抽题」改为「全程 LLM 驱动的对话式面试官」：支持简历结构化解析、有记忆的多轮对话、SSE 流式输出、项目/八股两环节状态机、结束时多维度总评。

**Architecture:** 新增 `resumes`、`interview_messages` 两张表并扩展 `mock_interviews`。`llm_client` 增加流式 `call_api_stream`（reqwest `bytes_stream` + SSE 行解析）与两个非流式 JSON 调用（简历解析、多维评分）。面试节奏由后端确定性状态机控制（轮数上限 + `[PHASE_DONE]` 提前切换标记），把易错逻辑（SSE 行解析、`[PHASE_DONE]` 剥离、环节决策）抽成纯函数单测。流式命令在 await 内通过 `AppHandle` 发 `interview-token` 事件，promise resolve 时返回本轮最终状态。

**Tech Stack:** Rust + Tauri 2 + sqlx(SQLite) + reqwest(stream) + futures-util + serde_json。

**约束：代码尽量精简**——沿用现有 `llm_client`/`db`/命令模式与错误处理风格；旧的模拟面试命令（`start_mock_interview` 等）本计划暂不删除，由 Plan 2B 前端切换后在收尾时清理。

---

## 文件结构

- 修改 `src-tauri/Cargo.toml`：reqwest 加 `stream` feature；新增 `futures-util`。
- 修改 `src-tauri/src/llm_client.rs`：新增 SSE 行解析纯函数 + `call_api_stream` + `parse_resume_llm` + `evaluate_interview`。
- 修改 `src-tauri/src/models.rs`：新增简历与面试相关结构体。
- 修改 `src-tauri/src/db.rs`：新增建表与 CRUD 辅助函数 + 单元测试。
- 修改 `src-tauri/src/lib.rs`：新增面试状态机纯函数 + 单测；新增 4 个命令并注册。

---

## 数据模型约定（跨任务一致性基准）

Rust 结构体（Task 3 在 models.rs 定义，后续任务引用这些名字）：

```rust
// 简历解析（LLM 输出 + 返回前端）
pub struct ResumeProject { pub name: String, pub role: String, pub summary: String, pub highlights: Vec<String> }
pub struct ParsedResume { pub candidate: String, pub projects: Vec<ResumeProject>, pub tech_stack: Vec<String> }
pub struct ResumeRecord { pub id: i64, pub candidate: String, pub projects: Vec<ResumeProject>, pub tech_stack: Vec<String> }

// 面试对话
pub struct InterviewMessage { pub role: String, pub phase: String, pub content: String, pub seq: i64 }
pub struct InterviewTurn { pub message: String, pub phase: String, pub finished: bool } // start/respond 命令返回
pub struct DimensionScores { pub project_depth: i32, pub fundamental_solidity: i32, pub communication: i32 }
pub struct InterviewReport2 { pub interview_id: i64, pub average_score: f64, pub dimension_scores: DimensionScores, pub summary: String, pub messages: Vec<InterviewMessage> }
```

phase 取值固定字符串：`"project"`、`"fundamental"`。role 取值：`"interviewer"`、`"candidate"`。

事件：`interview-token` 负载 `{ interviewId: i64, chunk: String }`（仅流式文本块）。本轮最终状态（phase/finished）由命令返回值 `InterviewTurn` 给出，前端据此处理，不再单独发 turn-done 事件（精简）。

---

## Task 1: 添加流式依赖

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: 修改依赖**

把 reqwest 那行改为带 `stream` feature，并在其下新增 `futures-util`：

```toml
reqwest = { version = "0.12", features = ["json", "stream"] }
futures-util = "0.3"
```

- [ ] **Step 2: 验证可拉取编译**

Run: `cd src-tauri && cargo build`
Expected: 依赖成功下载并编译通过（首次会拉取 futures-util；现有代码不受影响）。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "build: reqwest 启用 stream feature 并引入 futures-util"
```

---

## Task 2: SSE 行解析纯函数 + 测试

**Files:**
- Modify: `src-tauri/src/llm_client.rs`

- [ ] **Step 1: 写失败测试**

在 `llm_client.rs` 末尾新增测试模块（文件当前没有 tests 模块，新增一个）：

```rust
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
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test parse_sse_line`
Expected: 编译失败——`parse_sse_line` / `SseEvent` 未定义。

- [ ] **Step 3: 实现枚举与解析函数**

在 `llm_client.rs` 顶部 `clean_json` 函数附近新增：

```rust
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
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test parse_sse_line`
Expected: 3 个测试 PASS。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/llm_client.rs
git commit -m "feat: 新增 SSE 行解析纯函数与测试"
```

---

## Task 3: 流式 LLM 调用 call_api_stream

**Files:**
- Modify: `src-tauri/src/llm_client.rs`

- [ ] **Step 1: 实现流式调用**

在 `llm_client.rs` 顶部 import 处加 `use futures_util::StreamExt;`（与现有 `use reqwest::Client;` 同区）。新增函数：

```rust
// 流式调用：messages 为完整 chat 消息数组；on_token 对每个文本增量回调一次；返回拼接后的完整文本。
pub async fn call_api_stream<F: FnMut(&str)>(
    api_url: &str,
    api_key: &str,
    model: &str,
    messages: Vec<Value>,
    temperature: f64,
    max_tokens: u32,
    mut on_token: F,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("API Key 未配置，请点击左下角「设置」填写。".into());
    }

    let client = Client::new();
    let request_body = json!({
        "model": model,
        "messages": messages,
        "temperature": temperature,
        "max_tokens": max_tokens,
        "stream": true
    });

    let response = client
        .post(api_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("网络请求失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("API 请求失败，状态码: {}，详情: {}", status, error_text));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut full = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("读取流失败: {}", e))?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        // 按行处理；最后一段可能不完整，留在 buffer 里等下次
        while let Some(pos) = buffer.find('\n') {
            let line: String = buffer.drain(..=pos).collect();
            match parse_sse_line(&line) {
                SseEvent::Delta(token) => {
                    on_token(&token);
                    full.push_str(&token);
                }
                SseEvent::Done => return Ok(full),
                SseEvent::Other => {}
            }
        }
    }

    // 流结束但没遇到 [DONE]：处理 buffer 残余行
    if !buffer.trim().is_empty() {
        if let SseEvent::Delta(token) = parse_sse_line(&buffer) {
            on_token(&token);
            full.push_str(&token);
        }
    }

    Ok(full)
}
```

- [ ] **Step 2: 编译验证**

Run: `cd src-tauri && cargo build`
Expected: 编译通过（暂无调用者，会有 dead_code 警告，可接受）。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/llm_client.rs
git commit -m "feat: 新增 SSE 流式 LLM 调用 call_api_stream"
```

---

## Task 4: 简历与面试数据模型

**Files:**
- Modify: `src-tauri/src/models.rs`

- [ ] **Step 1: 新增结构体**

在 `models.rs` 末尾追加（保持与本计划「数据模型约定」一致）：

```rust
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
    pub messages: Vec<InterviewMessage>,
}
```

- [ ] **Step 2: 编译验证**

Run: `cd src-tauri && cargo build`
Expected: 编译通过（dead_code 警告可接受）。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/models.rs
git commit -m "feat: 新增简历与对话式面试数据模型"
```

---

## Task 5: 数据库表与 CRUD 辅助函数

**Files:**
- Modify: `src-tauri/src/db.rs`

- [ ] **Step 1: 在 init_db 建表 + 扩展 mock_interviews**

在 `init_db` 中 `wrong_book_manual` 建表之后、种子数据之前，加入：

```rust
sqlx::query(
    "CREATE TABLE IF NOT EXISTS resumes (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        created_at  TEXT NOT NULL DEFAULT (datetime('now','localtime')),
        raw_text    TEXT NOT NULL,
        candidate   TEXT NOT NULL DEFAULT '',
        projects    TEXT NOT NULL DEFAULT '[]',
        tech_stack  TEXT NOT NULL DEFAULT '[]'
    );"
)
.execute(&pool)
.await?;

sqlx::query(
    "CREATE TABLE IF NOT EXISTS interview_messages (
        id           INTEGER PRIMARY KEY AUTOINCREMENT,
        interview_id INTEGER NOT NULL REFERENCES mock_interviews(id) ON DELETE CASCADE,
        role         TEXT NOT NULL,
        phase        TEXT NOT NULL,
        content      TEXT NOT NULL,
        seq          INTEGER NOT NULL,
        created_at   TEXT NOT NULL DEFAULT (datetime('now','localtime'))
    );"
)
.execute(&pool)
.await?;

// 扩展 mock_interviews（幂等：列已存在时忽略错误，沿用本文件既有 ALTER 风格）
let _ = sqlx::query("ALTER TABLE mock_interviews ADD COLUMN resume_id INTEGER").execute(&pool).await;
let _ = sqlx::query("ALTER TABLE mock_interviews ADD COLUMN project_cap INTEGER NOT NULL DEFAULT 5").execute(&pool).await;
let _ = sqlx::query("ALTER TABLE mock_interviews ADD COLUMN fundamental_cap INTEGER NOT NULL DEFAULT 5").execute(&pool).await;
let _ = sqlx::query("ALTER TABLE mock_interviews ADD COLUMN dimension_scores TEXT NOT NULL DEFAULT '{}'").execute(&pool).await;
let _ = sqlx::query("ALTER TABLE mock_interviews ADD COLUMN phase TEXT NOT NULL DEFAULT 'project'").execute(&pool).await;
```

- [ ] **Step 2: 写失败测试**

在 `mod tests` 内追加（覆盖建表 + 消息写入读取 + phase 计数）：

```rust
#[tokio::test]
async fn resume_and_interview_message_roundtrip() {
    let db_path = test_db_path("interview2");
    let pool = init_db(db_path.clone()).await.unwrap();

    let resume_id = create_resume(&pool, "raw resume text", "张三", "[]", r#"["Rust"]"#)
        .await
        .unwrap();
    assert!(resume_id > 0);

    let iv_id = create_interview2(&pool, resume_id, 5, 5, "Rust").await.unwrap();
    assert!(iv_id > 0);

    add_interview_message(&pool, iv_id, "interviewer", "project", "介绍下你的项目？").await.unwrap();
    add_interview_message(&pool, iv_id, "candidate", "project", "我做了一个...").await.unwrap();
    add_interview_message(&pool, iv_id, "interviewer", "project", "为什么这样设计？").await.unwrap();

    let msgs = get_interview_messages(&pool, iv_id).await.unwrap();
    assert_eq!(msgs.len(), 3);
    assert_eq!(msgs[0].seq, 1);
    assert_eq!(msgs[2].seq, 3);

    // 面试官在 project 环节提了 2 个问题
    let asked = count_phase_questions(&pool, iv_id, "project").await.unwrap();
    assert_eq!(asked, 2);

    pool.close().await;
    let _ = std::fs::remove_file(db_path);
}
```

- [ ] **Step 3: 运行测试确认失败**

Run: `cd src-tauri && cargo test resume_and_interview_message_roundtrip`
Expected: 编译失败——相关函数未定义。

- [ ] **Step 4: 实现辅助函数**

在 `db.rs`（`mark_question_wrong` 附近、`mod tests` 之前）加：

```rust
pub async fn create_resume(
    pool: &SqlitePool,
    raw_text: &str,
    candidate: &str,
    projects_json: &str,
    tech_stack_json: &str,
) -> Result<i64, String> {
    sqlx::query_scalar::<_, i64>(
        "INSERT INTO resumes (raw_text, candidate, projects, tech_stack)
         VALUES (?, ?, ?, ?) RETURNING id",
    )
    .bind(raw_text)
    .bind(candidate)
    .bind(projects_json)
    .bind(tech_stack_json)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("保存简历失败: {}", e))
}

// 读取简历的 (candidate, projects_json, tech_stack_json)
pub async fn get_resume_raw(pool: &SqlitePool, id: i64) -> Result<(String, String, String), String> {
    sqlx::query_as::<_, (String, String, String)>(
        "SELECT candidate, projects, tech_stack FROM resumes WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("读取简历失败: {}", e))
}

pub async fn create_interview2(
    pool: &SqlitePool,
    resume_id: i64,
    project_cap: i32,
    fundamental_cap: i32,
    tags: &str,
) -> Result<i64, String> {
    sqlx::query_scalar::<_, i64>(
        "INSERT INTO mock_interviews (tags, question_count, status, resume_id, project_cap, fundamental_cap, phase)
         VALUES (?, 0, 'active', ?, ?, ?, 'project') RETURNING id",
    )
    .bind(tags)
    .bind(resume_id)
    .bind(project_cap)
    .bind(fundamental_cap)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("创建面试失败: {}", e))
}

pub async fn add_interview_message(
    pool: &SqlitePool,
    interview_id: i64,
    role: &str,
    phase: &str,
    content: &str,
) -> Result<(), String> {
    let next_seq: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(seq), 0) + 1 FROM interview_messages WHERE interview_id = ?",
    )
    .bind(interview_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("计算消息序号失败: {}", e))?;

    sqlx::query(
        "INSERT INTO interview_messages (interview_id, role, phase, content, seq)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(interview_id)
    .bind(role)
    .bind(phase)
    .bind(content)
    .bind(next_seq)
    .execute(pool)
    .await
    .map_err(|e| format!("保存对话消息失败: {}", e))?;
    Ok(())
}

pub async fn get_interview_messages(
    pool: &SqlitePool,
    interview_id: i64,
) -> Result<Vec<crate::models::InterviewMessage>, String> {
    sqlx::query_as::<_, (String, String, String, i64)>(
        "SELECT role, phase, content, seq FROM interview_messages
         WHERE interview_id = ? ORDER BY seq ASC",
    )
    .bind(interview_id)
    .fetch_all(pool)
    .await
    .map(|rows| {
        rows.into_iter()
            .map(|(role, phase, content, seq)| crate::models::InterviewMessage { role, phase, content, seq })
            .collect()
    })
    .map_err(|e| format!("读取对话失败: {}", e))
}

// 某环节面试官已提问的轮数
pub async fn count_phase_questions(pool: &SqlitePool, interview_id: i64, phase: &str) -> Result<i64, String> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM interview_messages
         WHERE interview_id = ? AND phase = ? AND role = 'interviewer'",
    )
    .bind(interview_id)
    .bind(phase)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("统计提问数失败: {}", e))
}

pub async fn get_interview_phase(pool: &SqlitePool, interview_id: i64) -> Result<(String, i32, i32, i64), String> {
    // 返回 (phase, project_cap, fundamental_cap, resume_id)
    sqlx::query_as::<_, (String, i32, i32, i64)>(
        "SELECT phase, project_cap, fundamental_cap, COALESCE(resume_id, 0) FROM mock_interviews WHERE id = ?",
    )
    .bind(interview_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("读取面试状态失败: {}", e))
}

pub async fn set_interview_phase(pool: &SqlitePool, interview_id: i64, phase: &str) -> Result<(), String> {
    sqlx::query("UPDATE mock_interviews SET phase = ? WHERE id = ?")
        .bind(phase)
        .bind(interview_id)
        .execute(pool)
        .await
        .map_err(|e| format!("更新环节失败: {}", e))?;
    Ok(())
}

pub async fn finish_interview2(
    pool: &SqlitePool,
    interview_id: i64,
    average_score: f64,
    dimension_scores_json: &str,
    summary: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE mock_interviews
         SET ended_at = datetime('now','localtime'), average_score = ?, dimension_scores = ?, summary = ?, status = 'finished'
         WHERE id = ?",
    )
    .bind(average_score)
    .bind(dimension_scores_json)
    .bind(summary)
    .bind(interview_id)
    .execute(pool)
    .await
    .map_err(|e| format!("保存面试总结失败: {}", e))?;
    Ok(())
}
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cd src-tauri && cargo test resume_and_interview_message_roundtrip`
Expected: PASS。再跑 `cargo test` 确认既有测试全绿。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/db.rs
git commit -m "feat: db 层新增简历/对话式面试表与 CRUD 辅助函数"
```

---

## Task 6: 简历解析与多维评分（非流式 LLM）

**Files:**
- Modify: `src-tauri/src/llm_client.rs`

- [ ] **Step 1: 实现简历解析**

新增（沿用现有 `clean_json` + `call_api` 模式）：

```rust
pub async fn parse_resume_llm(
    api_url: &str,
    api_key: &str,
    model: &str,
    raw_text: &str,
) -> Result<crate::models::ParsedResume, String> {
    let system_prompt = concat!(
        "你是简历解析助手。从候选人简历文本中提取结构化信息。",
        "只返回 JSON，不要任何 Markdown 或多余文字，格式：",
        r#"{"candidate":"姓名或标题","projects":[{"name":"项目名","role":"担任角色","summary":"一句话简介","highlights":["技术亮点1","亮点2"]}],"tech_stack":["技能1","技能2"]}"#
    );
    let raw = call_api(api_url, api_key, model, system_prompt, raw_text, 0.2, 2048).await?;
    serde_json::from_str(clean_json(&raw))
        .map_err(|e| format!("简历解析 JSON 失败: {}，原始内容: {}", e, raw))
}
```

- [ ] **Step 2: 实现多维评分**

```rust
// 基于完整对话记录给出三维评分 + 文字复盘
pub async fn evaluate_interview(
    api_url: &str,
    api_key: &str,
    model: &str,
    transcript: &str,
) -> Result<(crate::models::DimensionScores, String), String> {
    let system_prompt = concat!(
        "你是资深技术面试官，对一场模拟面试做复盘。基于完整对话记录，给出三个维度 0-100 的评分与一段中文总结（150 字内，含薄弱点与改进建议）。",
        "只返回 JSON：",
        r#"{"project_depth":85,"fundamental_solidity":70,"communication":80,"summary":"..."}"#
    );
    let raw = call_api(api_url, api_key, model, system_prompt, transcript, 0.3, 1024).await?;
    let v: Value = serde_json::from_str(clean_json(&raw))
        .map_err(|e| format!("评分 JSON 解析失败: {}，原始内容: {}", e, raw))?;
    let scores = crate::models::DimensionScores {
        project_depth: v["project_depth"].as_i64().unwrap_or(0).clamp(0, 100) as i32,
        fundamental_solidity: v["fundamental_solidity"].as_i64().unwrap_or(0).clamp(0, 100) as i32,
        communication: v["communication"].as_i64().unwrap_or(0).clamp(0, 100) as i32,
    };
    let summary = v["summary"].as_str().unwrap_or("").trim().to_string();
    Ok((scores, summary))
}
```

- [ ] **Step 3: 编译验证**

Run: `cd src-tauri && cargo build`
Expected: 编译通过（dead_code 警告可接受）。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/llm_client.rs
git commit -m "feat: 新增简历解析与面试多维评分 LLM 调用"
```

---

## Task 7: 面试状态机纯函数 + 测试

**Files:**
- Modify: `src-tauri/src/lib.rs`

说明：把「`[PHASE_DONE]` 剥离」和「下一步环节决策」抽成纯函数，便于单测。

- [ ] **Step 1: 写失败测试**

在 `lib.rs` 末尾新增测试模块（若已存在 tests 模块则并入）：

```rust
#[cfg(test)]
mod phase_tests {
    use super::*;

    #[test]
    fn strip_phase_done_detects_and_removes_marker() {
        let (text, done) = strip_phase_done("很好，项目部分聊得差不多了。\n[PHASE_DONE]");
        assert!(done);
        assert!(!text.contains("[PHASE_DONE]"));
        assert!(text.contains("项目部分"));

        let (text2, done2) = strip_phase_done("继续说说你的下一个项目？");
        assert!(!done2);
        assert_eq!(text2, "继续说说你的下一个项目？");
    }

    #[test]
    fn decide_phase_advances_on_cap() {
        // project 环节，已问满 cap → 切 fundamental
        assert_eq!(decide_phase("project", 5, 5, 0, 5, false).0, "fundamental");
        // project 环节，未满且无 PHASE_DONE → 留 project
        assert_eq!(decide_phase("project", 2, 5, 0, 5, false).0, "project");
        // project 环节，收到 PHASE_DONE → 切 fundamental
        assert_eq!(decide_phase("project", 2, 5, 0, 5, true).0, "fundamental");
    }

    #[test]
    fn decide_phase_finishes_when_fundamental_full() {
        // fundamental 环节问满 cap → finished=true
        let (_phase, finished) = decide_phase("fundamental", 5, 5, 5, 5, false);
        assert!(finished);
        // fundamental 未满 → 不结束
        let (_p, fin2) = decide_phase("fundamental", 5, 5, 2, 5, false);
        assert!(!fin2);
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib phase_tests`
Expected: 编译失败——`strip_phase_done` / `decide_phase` 未定义。

- [ ] **Step 3: 实现纯函数**

在 `lib.rs`（命令定义区附近、模块级）新增：

```rust
const PHASE_DONE_MARK: &str = "[PHASE_DONE]";

// 剥离面试官输出里的 [PHASE_DONE] 标记，返回 (清理后的文本, 是否检测到标记)
fn strip_phase_done(text: &str) -> (String, bool) {
    if let Some(idx) = text.find(PHASE_DONE_MARK) {
        let mut cleaned = String::with_capacity(text.len());
        cleaned.push_str(&text[..idx]);
        cleaned.push_str(&text[idx + PHASE_DONE_MARK.len()..]);
        (cleaned.trim().to_string(), true)
    } else {
        (text.trim().to_string(), false)
    }
}

// 决定下一次面试官提问所属环节，以及面试是否应结束。
// 入参：当前 phase、project 已问轮数、project 上限、fundamental 已问轮数、fundamental 上限、本轮是否收到 PHASE_DONE。
// 返回：(下一环节, 是否结束)
fn decide_phase(
    current_phase: &str,
    project_used: i32,
    project_cap: i32,
    fundamental_used: i32,
    fundamental_cap: i32,
    phase_done: bool,
) -> (String, bool) {
    if current_phase == "project" {
        if phase_done || project_used >= project_cap {
            // 项目环节结束，转八股；若八股上限为 0 则直接结束
            if fundamental_cap <= 0 {
                return ("fundamental".into(), true);
            }
            return ("fundamental".into(), false);
        }
        ("project".into(), false)
    } else {
        // fundamental 环节
        if phase_done || fundamental_used >= fundamental_cap {
            return ("fundamental".into(), true);
        }
        ("fundamental".into(), false)
    }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib phase_tests`
Expected: 3 个测试 PASS。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: 新增面试环节状态机纯函数与测试"
```

---

## Task 8: parse_resume 命令

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 在 import 处补充模型类型**

`lib.rs` 顶部 `use crate::models::{...}` 列表中加入 `ParsedResume, ResumeRecord, InterviewMessage, InterviewTurn, DimensionScores, InterviewReport2`（按现有多行 import 风格补齐，逗号分隔）。

- [ ] **Step 2: 实现 parse_resume 命令**

在模拟面试命令区（`finish_mock_interview` 之后）新增：

```rust
#[tauri::command]
async fn parse_resume(
    raw_text: String,
    pool: tauri::State<'_, SqlitePool>,
    config: tauri::State<'_, Mutex<AppConfig>>,
) -> Result<ResumeRecord, String> {
    if raw_text.trim().is_empty() {
        return Err("简历内容为空，请确认 PDF 已正确解析。".into());
    }
    let (api_url, api_key, model) = {
        let cfg = config.lock().map_err(|e| e.to_string())?;
        (cfg.api_url.clone(), cfg.api_key.clone(), cfg.model.clone())
    };

    let parsed: ParsedResume =
        llm_client::parse_resume_llm(&api_url, &api_key, &model, &raw_text).await?;

    let projects_json = serde_json::to_string(&parsed.projects).unwrap_or_else(|_| "[]".into());
    let tech_stack_json = serde_json::to_string(&parsed.tech_stack).unwrap_or_else(|_| "[]".into());

    let id = db::create_resume(&pool, &raw_text, &parsed.candidate, &projects_json, &tech_stack_json).await?;

    Ok(ResumeRecord {
        id,
        candidate: parsed.candidate,
        projects: parsed.projects,
        tech_stack: parsed.tech_stack,
    })
}
```

- [ ] **Step 3: 编译验证**

Run: `cd src-tauri && cargo build`
Expected: 编译通过。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: 新增 parse_resume 命令"
```

---

## Task 9: start_interview 与 interview_respond（流式）

**Files:**
- Modify: `src-tauri/src/lib.rs`

说明：两命令共用一个内部 helper 构建消息、流式生成、剥离标记、落库、发事件。

- [ ] **Step 1: 实现共用 helper + 两个命令**

在 `lib.rs` 模拟面试命令区新增。注意：`app.emit` 需要 `use tauri::Emitter;`（文件顶部已有，确认即可）。

```rust
// 构建某环节的 system prompt
fn build_interviewer_system(phase: &str, resume_brief: &str, used: i32, cap: i32) -> String {
    let remaining = (cap - used).max(0);
    let phase_cn = if phase == "project" { "项目经历" } else { "技术八股（基础原理）" };
    format!(
        "你是一名资深技术面试官，正在进行模拟面试的【{phase_cn}】环节。\
        候选人简历摘要：{resume_brief}。\
        要求：每次只问一个问题；问题要顺着候选人上一轮回答自然深入或转向；语气专业简洁，不要寒暄过多；不要给出答案或点评。\
        本环节还可进行约 {remaining} 轮。若你认为本环节已充分考察，可在回复最后另起一行输出 {mark} 表示提前结束本环节。\
        {phase_extra}",
        phase_cn = phase_cn,
        resume_brief = resume_brief,
        remaining = remaining,
        mark = PHASE_DONE_MARK,
        phase_extra = if phase == "project" {
            "项目环节：围绕候选人真实项目追问技术选型、难点、权衡、量化结果。"
        } else {
            "八股环节：围绕候选人技术栈考察底层原理、常见考点，由浅入深。"
        }
    )
}

// 取简历摘要文本（候选人 + 技术栈 + 项目名），用于注入提示词
async fn resume_brief(pool: &SqlitePool, resume_id: i64) -> String {
    match db::get_resume_raw(pool, resume_id).await {
        Ok((candidate, projects_json, tech_json)) => {
            let projects: Vec<crate::models::ResumeProject> =
                serde_json::from_str(&projects_json).unwrap_or_default();
            let tech: Vec<String> = serde_json::from_str(&tech_json).unwrap_or_default();
            let proj_names: Vec<String> = projects.iter().map(|p| p.name.clone()).collect();
            format!("候选人 {}；技术栈 [{}]；项目 [{}]", candidate, tech.join("、"), proj_names.join("、"))
        }
        Err(_) => String::new(),
    }
}

// 生成一轮面试官提问：组装消息→流式生成（发 interview-token 事件）→剥离 PHASE_DONE→落库→推进状态机
async fn run_interviewer_turn(
    pool: &SqlitePool,
    app: &tauri::AppHandle,
    api_url: &str,
    api_key: &str,
    model: &str,
    interview_id: i64,
) -> Result<InterviewTurn, String> {
    // 注：`app.emit` 依赖文件顶部已有的 `use tauri::Emitter;`（lib.rs:17），此处不再重复 import。
    let (current_phase, project_cap, fundamental_cap, resume_id) =
        db::get_interview_phase(pool, interview_id).await?;
    let used = db::count_phase_questions(pool, interview_id, &current_phase).await? as i32;

    let brief = resume_brief(pool, resume_id).await;
    let cap = if current_phase == "project" { project_cap } else { fundamental_cap };
    let system = build_interviewer_system(&current_phase, &brief, used, cap);

    // 组装消息：system + 历史（interviewer→assistant, candidate→user）
    let history = db::get_interview_messages(pool, interview_id).await?;
    let mut messages: Vec<serde_json::Value> = vec![serde_json::json!({"role":"system","content":system})];
    for m in &history {
        let role = if m.role == "interviewer" { "assistant" } else { "user" };
        messages.push(serde_json::json!({"role": role, "content": m.content}));
    }
    // 若是本环节第一问且无历史，给个起始 user 提示，促使模型开口
    if history.is_empty() {
        messages.push(serde_json::json!({"role":"user","content":"请开始面试，先做简短开场再提出第一个问题。"}));
    }

    let app_for_cb = app.clone();
    let full = llm_client::call_api_stream(
        api_url, api_key, model, messages, 0.6, 1024,
        move |token| {
            let _ = app_for_cb.emit("interview-token", serde_json::json!({
                "interviewId": interview_id,
                "chunk": token,
            }));
        },
    )
    .await?;

    let (clean_text, phase_done) = strip_phase_done(&full);
    let message = if clean_text.is_empty() { "（面试官沉默了，请稍后重试）".to_string() } else { clean_text };

    // 落库本轮面试官提问
    db::add_interview_message(pool, interview_id, "interviewer", &current_phase, &message).await?;

    // 推进状态机：基于「问完这轮后」的计数判断下一环节/是否结束
    let used_after = used + 1;
    let (project_used, fundamental_used) = if current_phase == "project" {
        (used_after, db::count_phase_questions(pool, interview_id, "fundamental").await? as i32)
    } else {
        (db::count_phase_questions(pool, interview_id, "project").await? as i32, used_after)
    };
    let (next_phase, finished) = decide_phase(
        &current_phase, project_used, project_cap, fundamental_used, fundamental_cap, phase_done,
    );
    if next_phase != current_phase {
        db::set_interview_phase(pool, interview_id, &next_phase).await?;
    }

    Ok(InterviewTurn { message, phase: current_phase, finished })
}

#[tauri::command]
async fn start_interview(
    resume_id: i64,
    project_cap: i32,
    fundamental_cap: i32,
    pool: tauri::State<'_, SqlitePool>,
    config: tauri::State<'_, Mutex<AppConfig>>,
    app: tauri::AppHandle,
) -> Result<(i64, InterviewTurn), String> {
    let (api_url, api_key, model) = {
        let cfg = config.lock().map_err(|e| e.to_string())?;
        (cfg.api_url.clone(), cfg.api_key.clone(), cfg.model.clone())
    };
    let pc = project_cap.clamp(1, 20);
    let fc = fundamental_cap.clamp(0, 20);
    let interview_id = db::create_interview2(&pool, resume_id, pc, fc, "").await?;
    let turn = run_interviewer_turn(&pool, &app, &api_url, &api_key, &model, interview_id).await?;
    Ok((interview_id, turn))
}

#[tauri::command]
async fn interview_respond(
    interview_id: i64,
    answer: String,
    pool: tauri::State<'_, SqlitePool>,
    config: tauri::State<'_, Mutex<AppConfig>>,
    app: tauri::AppHandle,
) -> Result<InterviewTurn, String> {
    let (api_url, api_key, model) = {
        let cfg = config.lock().map_err(|e| e.to_string())?;
        (cfg.api_url.clone(), cfg.api_key.clone(), cfg.model.clone())
    };
    // 落库候选人回答（用当前环节）
    let (current_phase, _pc, _fc, _rid) = db::get_interview_phase(&pool, interview_id).await?;
    let ans = if answer.trim().is_empty() { "（跳过未作答）".to_string() } else { answer.trim().to_string() };
    db::add_interview_message(&pool, interview_id, "candidate", &current_phase, &ans).await?;

    run_interviewer_turn(&pool, &app, &api_url, &api_key, &model, interview_id).await
}
```

- [ ] **Step 2: 编译验证**

Run: `cd src-tauri && cargo build`
Expected: 编译通过。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: 新增 start_interview / interview_respond 流式命令"
```

---

## Task 10: finish_interview 命令 + 注册全部命令

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 实现 finish_interview**

```rust
#[tauri::command]
async fn finish_interview(
    interview_id: i64,
    pool: tauri::State<'_, SqlitePool>,
    config: tauri::State<'_, Mutex<AppConfig>>,
) -> Result<InterviewReport2, String> {
    let messages = db::get_interview_messages(&pool, interview_id).await?;

    // 候选人是否有过有效作答
    let answered = messages.iter().any(|m| m.role == "candidate" && m.content.trim() != "（跳过未作答）" && !m.content.trim().is_empty());

    let (scores, summary) = if !answered {
        (
            DimensionScores { project_depth: 0, fundamental_solidity: 0, communication: 0 },
            "本次面试没有任何有效作答，无法评估表现。建议正式作答后再生成复盘。".to_string(),
        )
    } else {
        let transcript = messages.iter()
            .map(|m| {
                let who = if m.role == "interviewer" { "面试官" } else { "候选人" };
                format!("[{}] {}: {}", m.phase, who, m.content)
            })
            .collect::<Vec<_>>()
            .join("\n");
        let (api_url, api_key, model) = {
            let cfg = config.lock().map_err(|e| e.to_string())?;
            (cfg.api_url.clone(), cfg.api_key.clone(), cfg.model.clone())
        };
        llm_client::evaluate_interview(&api_url, &api_key, &model, &transcript)
            .await
            .unwrap_or_else(|_| (
                DimensionScores { project_depth: 60, fundamental_solidity: 60, communication: 60 },
                "评分服务暂时不可用，已记录对话。建议复盘低分环节。".to_string(),
            ))
    };

    let average_score =
        (scores.project_depth + scores.fundamental_solidity + scores.communication) as f64 / 3.0;
    let dim_json = serde_json::to_string(&scores).unwrap_or_else(|_| "{}".into());
    db::finish_interview2(&pool, interview_id, average_score, &dim_json, &summary).await?;

    Ok(InterviewReport2 {
        interview_id,
        average_score,
        dimension_scores: scores,
        summary,
        messages,
    })
}
```

- [ ] **Step 2: 注册 4 个新命令**

在 `tauri::generate_handler![` 列表里（`mark_question_wrong,` 之后或任意合适位置）加：

```rust
            parse_resume,
            start_interview,
            interview_respond,
            finish_interview,
```

- [ ] **Step 3: 编译 + 跑全部测试**

Run: `cd src-tauri && cargo build && cargo test`
Expected: 编译通过；所有测试 PASS（既有 + 本计划新增的 SSE/phase/db 测试）。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: 新增 finish_interview 命令并注册对话式面试命令"
```

---

## 完成标准（Plan 2A）

- `cargo test` 全绿（含新增：3 个 SSE 解析、3 个 phase 状态机、1 个简历/对话 roundtrip）。
- `cargo build` 无错误。
- 后端具备：简历解析、流式对话面试、项目/八股状态机、多维评分四项能力，命令已注册，等待 Plan 2B 前端接入。
- 旧模拟面试命令暂保留（2B 切换前端后再清理）。

---

## 自检备注（写计划时已核对）

- 事件名 `interview-token`、负载键 `interviewId`/`chunk` 与 Plan 2B 前端约定一致。
- `start_interview` 返回 `(i64, InterviewTurn)` 元组（interview_id + 首轮）；`interview_respond` 返回 `InterviewTurn`；`finish_interview` 返回 `InterviewReport2`。前端按此对接。
- `decide_phase` 在 `fundamental_cap <= 0` 时项目环节结束即直接 finished，避免空环节死循环。
- 状态机计数基于 `interview_messages` 中 `role='interviewer'` 的条数，`run_interviewer_turn` 落库后用 `used+1` 推进，逻辑自洽。
