# 阶段一：题库 CRUD 完整化 + 小优化 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让题库支持手动新增/编辑/详情查看，补齐错题本手动标记与题库导出，并把硬编码的 LLM 模型名改为可配置（修 bug）。

**Architecture:** 后端可测逻辑放进 `src-tauri/src/db.rs`（接收 `&SqlitePool`，沿用现有 `#[tokio::test]` 测试模式），`lib.rs` 仅保留薄 `#[tauri::command]` 包装并注册到 `generate_handler!`。前端新增一个三态合一（新增/编辑/详情）的 `QuestionModal.vue`，接入题库页；无前端测试框架，用手动验证步骤。

**Tech Stack:** Rust + Tauri 2 + sqlx(SQLite) + Vue 3 + TypeScript。

**约束：代码尽量精简**——复用现有命令模式与样式 token，不引入新依赖（导出/保存用已装的 `@tauri-apps/plugin-dialog`）。

---

## 文件结构

- 修改 `src-tauri/src/config.rs`：`AppConfig` 增 `model` 字段。
- 修改 `src-tauri/src/llm_client.rs`：`call_api` 与各公开函数增 `model: &str` 形参，去掉硬编码 `"deepseek-chat"`。
- 修改 `src-tauri/src/db.rs`：新增 `create_question` / `update_question` / `export_questions_json` / `mark_question_wrong` 函数及 `wrong_book_manual` 建表；新增对应 `#[tokio::test]`。
- 修改 `src-tauri/src/lib.rs`：新增命令包装并注册；改造 `get_wrong_questions` / `remove_from_wrong_book` 合并手动标记来源；更新 llm_client 调用处传 `model`。
- 新建 `src/components/ui/QuestionModal.vue`：三态合一题目弹窗。
- 修改 `src/components/QuestionLibrary.vue`：接入弹窗（新增/编辑/详情/导出/加入错题本）。
- 修改 `src/components/Settings.vue`：模型名输入框。

---

## Task 1: AppConfig 增加 model 字段

**Files:**
- Modify: `src-tauri/src/config.rs`

- [ ] **Step 1: 给 AppConfig 增 model 字段并设默认值**

把 `config.rs` 的结构体与 `Default` 改为：

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub api_key: String,
    pub api_url: String,
    #[serde(default = "default_model")]
    pub model: String,
}

fn default_model() -> String {
    "deepseek-chat".into()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_url: "https://zenmux.ai/api/v1/chat/completions".into(),
            model: default_model(),
        }
    }
}
```

`#[serde(default = "default_model")]` 保证旧的 `config.json`（没有 model 字段）也能反序列化成功。

- [ ] **Step 2: 编译验证**

Run: `cd src-tauri && cargo build`
Expected: 编译报错——`set_api_config`/各 llm_client 调用尚未更新。这是预期的，后续任务修复。先确认 `config.rs` 本身无语法错误（错误只出现在其它文件）。

---

## Task 2: llm_client 去掉硬编码模型名

**Files:**
- Modify: `src-tauri/src/llm_client.rs`

- [ ] **Step 1: call_api 增加 model 形参**

把 `call_api` 签名与 body 改为（仅展示改动处）：

```rust
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
    // ...以下不变
```

- [ ] **Step 2: 给每个公开函数加 model 形参并转发**

在以下函数的参数列表中，紧跟 `api_key: &str` 之后加入 `model: &str`，并把其内部对 `call_api(...)` 的调用补上 `model`：
- `generate_answer_and_explanation_with_tags`
- `generate_answer_and_explanation`（标注了 `#[allow(dead_code)]`，同样改以保持一致）
- `evaluate_essay_answer`
- `generate_single_question`
- `evaluate_mock_interview_answer`
- `summarize_mock_interview`

例如 `evaluate_essay_answer` 改为：

```rust
pub async fn evaluate_essay_answer(
    api_url: &str,
    api_key: &str,
    model: &str,
    content: &str,
    standard_answer: &str,
    user_answer: &str,
) -> Result<(i32, String), String> {
    // ...system_prompt / user_prompt 不变...
    let raw = call_api(api_url, api_key, model, system_prompt, &user_prompt, 0.3, 1024).await?;
    // ...以下不变
```

其余函数同理：保持原有 `system_prompt`/`user_prompt`/temperature/max_tokens 不变，仅在 `call_api(api_url, api_key, ...)` 中间插入 `model`。

- [ ] **Step 3: 编译验证**

Run: `cd src-tauri && cargo build`
Expected: 仍报错，但错误集中在 `lib.rs`（调用 llm_client 的命令尚未传 model）。`llm_client.rs` 内部应无错误。

---

## Task 3: lib.rs 更新 llm_client 调用处 + set_api_config 落地 model

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: set_api_config 接收并保存 model**

把 `set_api_config`（约 `lib.rs:31-42`）改为：

```rust
#[tauri::command]
fn set_api_config(
    api_key: String,
    api_url: String,
    model: String,
    config: tauri::State<'_, Mutex<AppConfig>>,
    config_dir: tauri::State<'_, ConfigDir>,
) -> Result<(), String> {
    let mut cfg = config.lock().map_err(|e| e.to_string())?;
    cfg.api_key = api_key;
    cfg.api_url = api_url;
    cfg.model = if model.trim().is_empty() { cfg.model.clone() } else { model };
    cfg.save(&config_dir.0)
}
```

- [ ] **Step 2: 所有 llm_client 调用处读取并传入 cfg.model**

在每个调用 llm_client 的命令里，凡是已经 `let (api_url, api_key) = { let cfg = config.lock()...; (cfg.api_url.clone(), cfg.api_key.clone()) };` 的地方，改为一并取出 model：

```rust
let (api_url, api_key, model) = {
    let cfg = config.lock().map_err(|e| e.to_string())?;
    (cfg.api_url.clone(), cfg.api_key.clone(), cfg.model.clone())
};
```

然后在对应的 `llm_client::xxx(&api_url, &api_key, ...)` 调用中，紧跟 `&api_key` 之后加 `&model`。涉及的命令（用 grep 定位 `llm_client::` 全部调用点）：
- `evaluate_answer`（调用 `evaluate_essay_answer`）
- `submit_mock_answer`（调用 `evaluate_mock_interview_answer`）
- `finish_mock_interview`（调用 `summarize_mock_interview`）
- `generate_questions_by_ai`（调用 `generate_single_question`）
- `import_questions_from_file`（调用 `generate_answer_and_explanation_with_tags`）

> 用 `Grep` 搜索 `llm_client::` 确认所有调用点都已补 `&model`，不要遗漏。注意 `import_questions_from_file` 内部是 `tokio::spawn`，需在 spawn 前把 model 一起 clone 进闭包。

- [ ] **Step 3: 编译验证**

Run: `cd src-tauri && cargo build`
Expected: PASS（无错误）。若仍报 model 相关错误，按提示补齐遗漏的调用点。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/config.rs src-tauri/src/llm_client.rs src-tauri/src/lib.rs
git commit -m "fix: LLM 模型名改为可配置，移除硬编码 deepseek-chat"
```

---

## Task 4: Settings.vue 增加模型名输入

**Files:**
- Modify: `src/components/Settings.vue`

- [ ] **Step 1: 增加 localModel 状态并在挂载时载入**

在 `<script setup>` 中，`const localUrl = ref("");` 之后加：

```ts
const localModel = ref("");
```

在 `onMounted` 的 try 块里，`localUrl.value = cfg.api_url ?? localUrl.value;` 之后加：

```ts
localModel.value = cfg.model ?? "";
```

- [ ] **Step 2: save() 传入 model**

把 `save()` 中的 invoke 改为：

```ts
await invoke("set_api_config", {
  apiKey: localKey.value,
  apiUrl: localUrl.value,
  model: localModel.value,
});
```

- [ ] **Step 3: 模板中 API URL 字段下方增加模型字段**

在 `API URL` 的 `.field` 块之后、`.actions` 之前插入：

```html
<div class="field">
  <label>模型名称</label>
  <input v-model="localModel" type="text" class="fr-input" placeholder="deepseek-chat" />
</div>
```

- [ ] **Step 4: 手动验证**

Run: `npm run tauri dev`
操作：进入设置页 → 模型名称框显示当前值 → 改成任意值 → 保存 → 显示「已保存」→ 重启应用后该值仍在。
Expected: 模型名持久化成功。

- [ ] **Step 5: Commit**

```bash
git add src/components/Settings.vue
git commit -m "feat: 设置页支持配置 LLM 模型名"
```

---

## Task 5: db.rs 新增 create_question / update_question

**Files:**
- Modify: `src-tauri/src/db.rs`

- [ ] **Step 1: 写失败测试**

在 `db.rs` 的 `mod tests` 内追加（沿用现有 `test_db_path` / `init_db` 模式）：

```rust
#[tokio::test]
async fn create_and_update_question_roundtrip() {
    let db_path = test_db_path("question-crud");
    let pool = init_db(db_path.clone()).await.unwrap();

    let id = create_question(
        &pool, "ESSAY", "什么是所有权？", None, "Rust", 2,
        "Rust 的所有权机制……", "解析……",
    )
    .await
    .unwrap();
    assert!(id > 0);

    update_question(
        &pool, id, "ESSAY", "什么是所有权与借用？", None, "Rust", 3,
        "更新后的答案", "更新后的解析",
    )
    .await
    .unwrap();

    let row: (String, i32) =
        sqlx::query_as("SELECT content, difficulty FROM questions WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row.0, "什么是所有权与借用？");
    assert_eq!(row.1, 3);

    pool.close().await;
    let _ = std::fs::remove_file(db_path);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test create_and_update_question_roundtrip`
Expected: 编译失败——`create_question` / `update_question` 未定义。

- [ ] **Step 3: 实现两个函数**

在 `db.rs`（`create_topic` 函数附近，`mod tests` 之前）加：

```rust
#[allow(clippy::too_many_arguments)]
pub async fn create_question(
    pool: &SqlitePool,
    question_type: &str,
    content: &str,
    options: Option<&str>,
    tags: &str,
    difficulty: i32,
    standard_answer: &str,
    explanation: &str,
) -> Result<i64, String> {
    let content = content.trim();
    if content.is_empty() {
        return Err("题目内容不能为空".into());
    }
    sqlx::query_scalar::<_, i64>(
        "INSERT INTO questions
            (question_type, content, options, tags, difficulty, standard_answer, explanation)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         RETURNING id",
    )
    .bind(question_type)
    .bind(content)
    .bind(options)
    .bind(tags)
    .bind(difficulty)
    .bind(standard_answer)
    .bind(explanation)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("新增题目失败（可能题干重复）: {}", e))
}

#[allow(clippy::too_many_arguments)]
pub async fn update_question(
    pool: &SqlitePool,
    id: i32,
    question_type: &str,
    content: &str,
    options: Option<&str>,
    tags: &str,
    difficulty: i32,
    standard_answer: &str,
    explanation: &str,
) -> Result<(), String> {
    let content = content.trim();
    if content.is_empty() {
        return Err("题目内容不能为空".into());
    }
    sqlx::query(
        "UPDATE questions
         SET question_type = ?, content = ?, options = ?, tags = ?,
             difficulty = ?, standard_answer = ?, explanation = ?
         WHERE id = ?",
    )
    .bind(question_type)
    .bind(content)
    .bind(options)
    .bind(tags)
    .bind(difficulty)
    .bind(standard_answer)
    .bind(explanation)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| format!("更新题目失败（可能题干与其它题重复）: {}", e))?;
    Ok(())
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test create_and_update_question_roundtrip`
Expected: PASS。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db.rs
git commit -m "feat: db 层新增题目增改函数与测试"
```

---

## Task 6: db.rs 新增题库导出

**Files:**
- Modify: `src-tauri/src/db.rs`

- [ ] **Step 1: 写失败测试**

在 `mod tests` 内追加：

```rust
#[tokio::test]
async fn export_questions_json_contains_seed() {
    let db_path = test_db_path("export");
    let pool = init_db(db_path.clone()).await.unwrap();

    let json = export_questions_json(&pool).await.unwrap();
    // init_db 注入了 3 道种子题，导出应为非空 JSON 数组且含已知题干
    assert!(json.trim_start().starts_with('['));
    assert!(json.contains("进程与线程"));

    pool.close().await;
    let _ = std::fs::remove_file(db_path);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test export_questions_json_contains_seed`
Expected: 编译失败——`export_questions_json` 未定义。

- [ ] **Step 3: 实现导出函数**

导出格式要与导入（`ImportQuestion`：`options` 为字符串数组）兼容，便于回流。在 `db.rs` 加：

```rust
pub async fn export_questions_json(pool: &SqlitePool) -> Result<String, String> {
    let rows = sqlx::query_as::<_, crate::models::Question>(
        "SELECT * FROM questions ORDER BY id ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("读取题库失败: {}", e))?;

    let items: Vec<serde_json::Value> = rows
        .iter()
        .map(|q| {
            // DB 中 options 存为 JSON 字符串；导出成数组以与导入格式对齐
            let options: serde_json::Value = q
                .options
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(serde_json::Value::Null);
            serde_json::json!({
                "question_type": q.question_type,
                "content": q.content,
                "options": options,
                "tags": q.tags,
                "difficulty": q.difficulty,
                "standard_answer": q.standard_answer,
                "explanation": q.explanation,
            })
        })
        .collect();

    serde_json::to_string_pretty(&items).map_err(|e| format!("序列化失败: {}", e))
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test export_questions_json_contains_seed`
Expected: PASS。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db.rs
git commit -m "feat: db 层新增题库 JSON 导出与测试"
```

---

## Task 7: db.rs 错题本手动标记（独立表，不污染统计）

**Files:**
- Modify: `src-tauri/src/db.rs`

- [ ] **Step 1: 在 init_db 建表 wrong_book_manual**

在 `init_db` 中，`mock_interview_turns` 建表之后、种子数据之前，加：

```rust
sqlx::query(
    "CREATE TABLE IF NOT EXISTS wrong_book_manual (
        question_id INTEGER PRIMARY KEY REFERENCES questions(id) ON DELETE CASCADE,
        created_at  TEXT NOT NULL DEFAULT (datetime('now','localtime'))
    );"
)
.execute(&pool)
.await?;
```

- [ ] **Step 2: 写失败测试**

在 `mod tests` 内追加：

```rust
#[tokio::test]
async fn manual_wrong_mark_and_clear() {
    let db_path = test_db_path("manual-wrong");
    let pool = init_db(db_path.clone()).await.unwrap();

    let id = create_question(
        &pool, "ESSAY", "手动标记测试题", None, "其他", 1, "答案", "解析",
    )
    .await
    .unwrap() as i32;

    mark_question_wrong(&pool, id).await.unwrap();
    let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM wrong_book_manual WHERE question_id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cnt, 1);

    // 重复标记应幂等
    mark_question_wrong(&pool, id).await.unwrap();
    let cnt2: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM wrong_book_manual WHERE question_id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cnt2, 1);

    pool.close().await;
    let _ = std::fs::remove_file(db_path);
}
```

- [ ] **Step 3: 运行测试确认失败**

Run: `cd src-tauri && cargo test manual_wrong_mark_and_clear`
Expected: 编译失败——`mark_question_wrong` 未定义。

- [ ] **Step 4: 实现 mark_question_wrong**

```rust
pub async fn mark_question_wrong(pool: &SqlitePool, question_id: i32) -> Result<(), String> {
    sqlx::query("INSERT OR IGNORE INTO wrong_book_manual (question_id) VALUES (?)")
        .bind(question_id)
        .execute(pool)
        .await
        .map_err(|e| format!("加入错题本失败: {}", e))?;
    Ok(())
}
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cd src-tauri && cargo test manual_wrong_mark_and_clear`
Expected: PASS。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/db.rs
git commit -m "feat: db 层新增手动错题标记表与函数"
```

---

## Task 8: get_wrong_questions 合并手动标记来源

**Files:**
- Modify: `src-tauri/src/lib.rs:99-137`

- [ ] **Step 1: 改造 get_wrong_questions 查询**

把 `get_wrong_questions` 的 SQL 改为用 UNION 汇总两个来源的 question_id，再聚合。手动标记但从未答过的题：`wrong_count` 计 0、`last_score` 取 -1（前端可识别为「未作答」）、`last_attempt` 取标记时间。

```rust
#[tauri::command]
async fn get_wrong_questions(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<WrongQuestion>, String> {
    sqlx::query_as::<_, WrongQuestion>(
        "SELECT
            q.id          AS question_id,
            q.content,
            q.question_type,
            q.tags,
            q.difficulty,
            q.standard_answer,
            q.explanation,
            COALESCE(r.wrong_count, 0)                       AS wrong_count,
            COALESCE(r.last_score, -1)                       AS last_score,
            COALESCE(r.last_attempt, m.created_at, '')       AS last_attempt,
            COALESCE(r.manually_added_count, 0)
                + CASE WHEN m.question_id IS NOT NULL THEN 1 ELSE 0 END
                                                             AS manually_added_count
         FROM questions q
         LEFT JOIN (
            SELECT question_id,
                   COUNT(*)              AS wrong_count,
                   MAX(score)            AS last_score,
                   MAX(created_at)       AS last_attempt,
                   SUM(manually_added)   AS manually_added_count
            FROM training_records
            WHERE score < 60 OR is_correct = 0 OR manually_added = 1
            GROUP BY question_id
         ) r ON r.question_id = q.id
         LEFT JOIN wrong_book_manual m ON m.question_id = q.id
         WHERE r.question_id IS NOT NULL OR m.question_id IS NOT NULL
         ORDER BY wrong_count DESC, last_attempt DESC"
    )
    .fetch_all(&*pool)
    .await
    .map_err(|e| format!("查询错题本失败: {}", e))
}
```

- [ ] **Step 2: remove_from_wrong_book 同时清手动标记**

把 `remove_from_wrong_book` 改为同时删两处：

```rust
#[tauri::command]
async fn remove_from_wrong_book(
    question_id: i32,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("DELETE FROM training_records WHERE question_id = ?")
        .bind(question_id)
        .execute(&*pool)
        .await
        .map_err(|e| format!("删除失败: {}", e))?;
    sqlx::query("DELETE FROM wrong_book_manual WHERE question_id = ?")
        .bind(question_id)
        .execute(&*pool)
        .await
        .map_err(|e| format!("删除失败: {}", e))?;
    Ok(())
}
```

- [ ] **Step 3: 编译验证**

Run: `cd src-tauri && cargo build`
Expected: PASS（`WrongQuestion.last_score` 为 `i32`，COALESCE 的 -1 兼容；其它字段类型不变）。

---

## Task 9: lib.rs 新增命令包装并注册

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 新增四个命令包装**

在题库命令区（`list_questions` 附近）加：

```rust
#[tauri::command]
async fn create_question(
    question_type: String,
    content: String,
    options: Option<String>,
    tags: String,
    difficulty: i32,
    standard_answer: String,
    explanation: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    db::create_question(
        &pool, &question_type, &content, options.as_deref(),
        &tags, difficulty, &standard_answer, &explanation,
    )
    .await
}

#[tauri::command]
async fn update_question(
    id: i32,
    question_type: String,
    content: String,
    options: Option<String>,
    tags: String,
    difficulty: i32,
    standard_answer: String,
    explanation: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    db::update_question(
        &pool, id, &question_type, &content, options.as_deref(),
        &tags, difficulty, &standard_answer, &explanation,
    )
    .await
}

#[tauri::command]
async fn export_questions(
    path: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<usize, String> {
    let json = db::export_questions_json(&pool).await?;
    std::fs::write(&path, &json).map_err(|e| format!("写入文件失败: {}", e))?;
    // 返回导出题数（用换行无关的方式：重新查 count 更稳）
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM questions")
        .fetch_one(&*pool)
        .await
        .map_err(|e| format!("统计失败: {}", e))?;
    Ok(count as usize)
}

#[tauri::command]
async fn mark_question_wrong(
    question_id: i32,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    db::mark_question_wrong(&pool, question_id).await
}
```

- [ ] **Step 2: 注册到 generate_handler!**

在 `lib.rs:1228` 的 `tauri::generate_handler![` 列表中，`count_questions,` 之后加：

```rust
            create_question,
            update_question,
            export_questions,
            mark_question_wrong,
```

- [ ] **Step 3: 编译 + 跑全部后端测试**

Run: `cd src-tauri && cargo build && cargo test`
Expected: 编译 PASS；所有测试 PASS（含 Task 5/6/7 的新测试与既有测试）。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: 新增题目增改/导出/错题标记命令并合并错题本来源"
```

---

## Task 10: 新建 QuestionModal.vue（新增/编辑/详情三态合一）

**Files:**
- Create: `src/components/ui/QuestionModal.vue`

- [ ] **Step 1: 创建组件**

复用现有设计 token 与 `Icon`。props：`mode`（`'create' | 'edit' | 'view'`）、`question`（编辑/详情时传入）、`tags`（可选考点列表）。emits：`close`、`saved`。新增/编辑共用表单，view 模式只读展示。

```vue
<script setup lang="ts">
import { reactive, ref, watch, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Icon from "./Icon.vue";

interface Question {
  id: number;
  question_type: string;
  content: string;
  options: string | null;
  tags: string;
  difficulty: number;
  standard_answer: string;
  explanation: string;
}

const props = defineProps<{
  mode: "create" | "edit" | "view";
  question?: Question | null;
  tags: string[];
}>();
const emit = defineEmits<{ (e: "close"): void; (e: "saved"): void }>();

const form = reactive({
  question_type: "ESSAY",
  content: "",
  options: [] as string[], // 仅选择题用
  tags: "",
  difficulty: 3,
  standard_answer: "",
  explanation: "",
});
const saving = ref(false);
const errorMsg = ref("");

const isView = computed(() => props.mode === "view");
const isChoice = computed(() => form.question_type !== "ESSAY");
const title = computed(() =>
  props.mode === "create" ? "新增题目" : props.mode === "edit" ? "编辑题目" : "题目详情"
);

function loadFromProp() {
  const q = props.question;
  if (!q) return;
  form.question_type = q.question_type;
  form.content = q.content;
  form.tags = q.tags;
  form.difficulty = q.difficulty;
  form.standard_answer = q.standard_answer;
  form.explanation = q.explanation;
  try {
    form.options = q.options ? JSON.parse(q.options) : [];
  } catch {
    form.options = [];
  }
}
watch(() => props.question, loadFromProp, { immediate: true });

function addOption() {
  form.options.push("");
}
function removeOption(i: number) {
  form.options.splice(i, 1);
}

async function save() {
  if (!form.content.trim()) {
    errorMsg.value = "题目内容不能为空";
    return;
  }
  saving.value = true;
  errorMsg.value = "";
  const optionsArg = isChoice.value && form.options.length
    ? JSON.stringify(form.options.filter((o) => o.trim()))
    : null;
  try {
    if (props.mode === "create") {
      await invoke("create_question", {
        questionType: form.question_type,
        content: form.content,
        options: optionsArg,
        tags: form.tags,
        difficulty: form.difficulty,
        standardAnswer: form.standard_answer,
        explanation: form.explanation,
      });
    } else if (props.mode === "edit" && props.question) {
      await invoke("update_question", {
        id: props.question.id,
        questionType: form.question_type,
        content: form.content,
        options: optionsArg,
        tags: form.tags,
        difficulty: form.difficulty,
        standardAnswer: form.standard_answer,
        explanation: form.explanation,
      });
    }
    emit("saved");
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <div class="modal-mask" @click.self="emit('close')">
    <div class="modal-card fr-card">
      <header class="modal-head">
        <h2>{{ title }}</h2>
        <button class="icon-btn" @click="emit('close')"><Icon name="X" :size="16" /></button>
      </header>

      <div class="modal-body">
        <div class="row">
          <label>题型</label>
          <select v-model="form.question_type" class="fr-input" :disabled="isView">
            <option value="ESSAY">简答</option>
            <option value="SINGLE">单选</option>
            <option value="MULTI">多选</option>
          </select>
        </div>

        <div class="row">
          <label>题目内容</label>
          <textarea v-model="form.content" class="fr-input" rows="3" :disabled="isView"></textarea>
        </div>

        <div v-if="isChoice" class="row">
          <label>选项</label>
          <div class="opt-list">
            <div v-for="(_, i) in form.options" :key="i" class="opt-item">
              <input v-model="form.options[i]" class="fr-input" :disabled="isView" placeholder="A. ..." />
              <button v-if="!isView" class="icon-btn" @click="removeOption(i)"><Icon name="X" :size="14" /></button>
            </div>
            <button v-if="!isView" class="fr-btn fr-btn-ghost" @click="addOption">
              <Icon name="Plus" :size="14" /><span>添加选项</span>
            </button>
          </div>
        </div>

        <div class="row two">
          <div>
            <label>考点标签（逗号分隔）</label>
            <input v-model="form.tags" class="fr-input" :disabled="isView" placeholder="Rust,所有权" />
          </div>
          <div>
            <label>难度 (1-5)</label>
            <input v-model.number="form.difficulty" type="number" min="1" max="5" class="fr-input" :disabled="isView" />
          </div>
        </div>

        <div class="row">
          <label>标准答案</label>
          <textarea v-model="form.standard_answer" class="fr-input" rows="3" :disabled="isView"></textarea>
        </div>

        <div class="row">
          <label>解析</label>
          <textarea v-model="form.explanation" class="fr-input" rows="3" :disabled="isView"></textarea>
        </div>

        <p v-if="errorMsg" class="error-msg">{{ errorMsg }}</p>
      </div>

      <footer class="modal-foot">
        <button class="fr-btn fr-btn-ghost" @click="emit('close')">{{ isView ? "关闭" : "取消" }}</button>
        <button v-if="!isView" class="fr-btn fr-btn-primary" :disabled="saving" @click="save">
          {{ saving ? "保存中..." : "保存" }}
        </button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.modal-mask {
  position: fixed; inset: 0; z-index: 50;
  background: rgba(0,0,0,0.4);
  display: flex; align-items: center; justify-content: center;
  padding: var(--sp-4);
}
.modal-card {
  width: 100%; max-width: 640px; max-height: 88vh;
  display: flex; flex-direction: column; gap: 0; overflow: hidden;
}
.modal-head, .modal-foot {
  display: flex; align-items: center; justify-content: space-between;
  padding: var(--sp-4) var(--sp-6);
}
.modal-head { border-bottom: 1px solid var(--border); }
.modal-foot { border-top: 1px solid var(--border); justify-content: flex-end; gap: var(--sp-2); }
.modal-head h2 { font-size: var(--fs-16); font-weight: var(--fw-semibold); }
.modal-body {
  padding: var(--sp-6);
  overflow-y: auto;
  display: flex; flex-direction: column; gap: var(--sp-4);
}
.row { display: flex; flex-direction: column; gap: 6px; }
.row.two { flex-direction: row; gap: var(--sp-4); }
.row.two > div { flex: 1; display: flex; flex-direction: column; gap: 6px; }
.row label { font-size: var(--fs-12); color: var(--text-muted); font-weight: var(--fw-medium); }
.opt-list { display: flex; flex-direction: column; gap: 6px; }
.opt-item { display: flex; gap: 6px; align-items: center; }
.icon-btn {
  width: 28px; height: 28px; border-radius: var(--radius-sm);
  color: var(--text-subtle); display: flex; align-items: center; justify-content: center;
}
.icon-btn:hover { color: var(--text); background: var(--surface-2); }
.error-msg { color: var(--danger); font-size: var(--fs-13); }
</style>
```

- [ ] **Step 2: 编译验证（类型检查）**

Run: `npm run build`
Expected: 无 TS/编译错误。

- [ ] **Step 3: Commit**

```bash
git add src/components/ui/QuestionModal.vue
git commit -m "feat: 新增题目弹窗组件（新增/编辑/详情三态合一）"
```

---

## Task 11: QuestionLibrary.vue 接入弹窗与导出、加入错题本

**Files:**
- Modify: `src/components/QuestionLibrary.vue`

- [ ] **Step 1: 引入弹窗与状态**

`<script setup>` 顶部 import 加：

```ts
import QuestionModal from "./ui/QuestionModal.vue";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
```

状态区加：

```ts
const modalMode = ref<"create" | "edit" | "view" | null>(null);
const modalQuestion = ref<Question | null>(null);

function openCreate() { modalQuestion.value = null; modalMode.value = "create"; }
function openEdit(q: Question) { modalQuestion.value = q; modalMode.value = "edit"; }
function openView(q: Question) { modalQuestion.value = q; modalMode.value = "view"; }
function closeModal() { modalMode.value = null; }
function onSaved() { modalMode.value = null; refresh(); }

async function handleExport() {
  const path = await saveDialog({
    defaultPath: "forgerust-questions.json",
    filters: [{ name: "JSON 题库", extensions: ["json"] }],
  });
  if (!path) return;
  try {
    const n = await invoke<number>("export_questions", { path });
    alert(`已导出 ${n} 道题到\n${path}`);
  } catch (e) {
    alert(e);
  }
}

async function handleMarkWrong(q: Question) {
  try {
    await invoke("mark_question_wrong", { questionId: q.id });
    alert("已加入错题本");
  } catch (e) {
    alert(e);
  }
}
```

- [ ] **Step 2: header 增加「新增题目」「导出」按钮**

把 header 里的导入按钮那段替换为一组按钮：

```html
<div class="head-actions">
  <button class="fr-btn fr-btn-ghost" @click="handleExport">
    <Icon name="Download" :size="14" /><span>导出</span>
  </button>
  <button class="fr-btn fr-btn-ghost" @click="openCreate">
    <Icon name="Plus" :size="14" /><span>新增题目</span>
  </button>
  <button class="fr-btn fr-btn-primary" @click="handleImport">
    <Icon name="Upload" :size="14" /><span>导入题库</span>
  </button>
</div>
```

并在 `<style>` 加：

```css
.head-actions { display: flex; gap: var(--sp-2); align-items: center; }
```

- [ ] **Step 3: 列表行增加 详情/编辑/加入错题本 按钮**

把 `.q-actions` 块替换为：

```html
<div class="q-actions">
  <button class="icon-btn" title="查看详情" @click="openView(q)">
    <Icon name="Eye" :size="14" />
  </button>
  <button class="icon-btn" title="编辑" @click="openEdit(q)">
    <Icon name="Pencil" :size="14" />
  </button>
  <button class="icon-btn" title="加入错题本" @click="handleMarkWrong(q)">
    <Icon name="BookmarkPlus" :size="14" />
  </button>
  <button class="icon-btn danger" title="删除" @click="handleDelete(q)">
    <Icon name="Trash2" :size="14" />
  </button>
</div>
```

并把原 `.icon-btn:hover` 的危险色样式改为只作用于 `.icon-btn.danger:hover`，普通 hover 用中性色：

```css
.icon-btn:hover { color: var(--text); background: var(--surface-2); }
.icon-btn.danger:hover { color: var(--danger); background: var(--danger-soft); }
```

- [ ] **Step 4: 模板末尾挂载弹窗**

在最外层 `</div>` 之前（`</template>` 内）加：

```html
<QuestionModal
  v-if="modalMode"
  :mode="modalMode"
  :question="modalQuestion"
  :tags="tags"
  @close="closeModal"
  @saved="onSaved"
/>
```

- [ ] **Step 5: 手动验证**

Run: `npm run tauri dev`
操作并确认：
1. 点「新增题目」→ 填简答题 → 保存 → 列表出现新题。
2. 对某题点编辑 → 改难度/内容 → 保存 → 列表更新。
3. 点详情（眼睛）→ 只读展示完整内容、选项、答案、解析。
4. 点「加入错题本」→ 提示成功 → 切到错题本页能看到该题（未作答显示，`last_score=-1`）。
5. 点「导出」→ 选保存路径 → 提示导出 N 题 → 该 JSON 可被「导入题库」重新导入。
Expected: 全部通过。

- [ ] **Step 6: Commit**

```bash
git add src/components/QuestionLibrary.vue
git commit -m "feat: 题库页接入新增/编辑/详情弹窗、导出与加入错题本"
```

---

## Task 12: 核对并提交工作区遗留改动

**Files:**
- 既有未提交改动：`src/components/AIGenerate.vue`、`src/components/Settings.vue`、`src/components/MockInterview.vue`、`src/styles/global.css`、`src-tauri/forgerust.db`

- [ ] **Step 1: 查看各文件 diff**

Run: `git diff src/components/AIGenerate.vue src/styles/global.css src/components/MockInterview.vue`
逐一阅读，确认是有意的改动（非误改、无调试残留）。

> 注意：`Settings.vue` 已在 Task 4 提交；若此处仍显示改动，是 Task 4 之外的旧改动，一并核对。

- [ ] **Step 2: 决定 forgerust.db 是否应被追踪**

Run: `git ls-files --error-unmatch src-tauri/forgerust.db`
- 若该 .db 已被 git 追踪：它是开发数据库，不应随业务代码提交。建议 `git rm --cached src-tauri/forgerust.db` 并在 `.gitignore` 加入 `src-tauri/forgerust.db`（与既有 scripts 忽略策略一致）。**执行前向用户确认**——若该库含用户想保留的种子题，则保留追踪。
- 若未被追踪：忽略。

- [ ] **Step 3: 提交确认无误的前端遗留改动**

```bash
git add src/components/AIGenerate.vue src/styles/global.css src/components/MockInterview.vue
git commit -m "chore: 提交此前未入库的前端改动"
```

> `MockInterview.vue` 阶段二会大改；此处仅把当前已存在的改动入库，作为阶段二重构的干净基线。

- [ ] **Step 4: 最终验证整轮通过**

Run: `cd src-tauri && cargo test` 然后 `npm run build`
Expected: 后端测试全 PASS，前端构建无错误。

---

## 完成标准

- 设置页可配置模型名并持久化；LLM 调用使用该模型（不再硬编码）。
- 题库页可新增、编辑、查看详情、删除、导出 JSON、把任意题加入错题本。
- 错题本能显示手动标记的题，且移除时一并清除；手动标记不污染 dashboard 统计。
- `cargo test` 全绿，`npm run build` 无错误。
- 工作区遗留改动已核对并入库，为阶段二提供干净基线。
