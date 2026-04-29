mod db;
mod models;
mod llm_client;
mod config;

use crate::config::AppConfig;
use crate::models::{EvaluateResponse, ImportQuestion, ImportResult, Question, ImportProgress, SaveRecordInput, WrongQuestion};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;
use tauri::Emitter;

// config_dir 单独作为 state，用 newtype 避免与其他 PathBuf 冲突
struct ConfigDir(PathBuf);

// ── 配置相关命令 ──────────────────────────────────────────

#[tauri::command]
fn get_api_config(
    config: tauri::State<'_, Mutex<AppConfig>>,
) -> Result<AppConfig, String> {
    config.lock().map(|c| c.clone()).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_api_config(
    api_key: String,
    api_url: String,
    config: tauri::State<'_, Mutex<AppConfig>>,
    config_dir: tauri::State<'_, ConfigDir>,
) -> Result<(), String> {
    let mut cfg = config.lock().map_err(|e| e.to_string())?;
    cfg.api_key = api_key;
    cfg.api_url = api_url;
    cfg.save(&config_dir.0)
}

// ── 训练记录命令 ──────────────────────────────────────────

#[tauri::command]
async fn save_training_session(
    records: Vec<SaveRecordInput>,
    tags: Vec<String>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    if records.is_empty() { return Ok(()); }

    let total = records.len() as i32;
    let correct = records.iter().filter(|r| {
        if r.skipped { return false; }
        r.is_correct.unwrap_or(false) || (!r.is_correct.is_some() && r.score >= 60)
    }).count() as i32;
    let skipped = records.iter().filter(|r| r.skipped).count() as i32;
    let scored: Vec<_> = records.iter().filter(|r| r.score >= 0).collect();
    let avg = if scored.is_empty() { 0.0 }
              else { scored.iter().map(|r| r.score as f64).sum::<f64>() / scored.len() as f64 };

    let session_id: i64 = sqlx::query_scalar(
        "INSERT INTO training_sessions (total_count, correct_count, average_score, skipped_count, tags)
         VALUES (?, ?, ?, ?, ?) RETURNING id"
    )
    .bind(total)
    .bind(correct)
    .bind(avg)
    .bind(skipped)
    .bind(tags.join(","))
    .fetch_one(&*pool)
    .await
    .map_err(|e| format!("保存训练会话失败: {}", e))?;

    for r in &records {
        sqlx::query(
            "INSERT INTO training_records
                (session_id, question_id, user_answer, score, is_correct, skipped, manually_added, time_spent)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(session_id)
        .bind(r.question_id)
        .bind(&r.user_answer)
        .bind(r.score)
        .bind(r.is_correct.map(|b| b as i32))
        .bind(r.skipped as i32)
        .bind(r.manually_added as i32)
        .bind(r.time_spent)
        .execute(&*pool)
        .await
        .map_err(|e| format!("保存题目记录失败: {}", e))?;
    }
    Ok(())
}

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
            COUNT(*)                                        AS wrong_count,
            MAX(r.score)                                    AS last_score,
            MAX(r.created_at)                               AS last_attempt,
            SUM(r.manually_added)                           AS manually_added_count
         FROM training_records r
         JOIN questions q ON q.id = r.question_id
         WHERE r.score < 60 OR r.is_correct = 0 OR r.manually_added = 1
         GROUP BY q.id
         ORDER BY wrong_count DESC, last_attempt DESC"
    )
    .fetch_all(&*pool)
    .await
    .map_err(|e| format!("查询错题本失败: {}", e))
}

#[tauri::command]
async fn generate_interview_from_ids(
    question_ids: Vec<i32>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<Question>, String> {
    if question_ids.is_empty() {
        return Err("没有可练习的错题".into());
    }
    let placeholders = question_ids.iter().enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT * FROM questions WHERE id IN ({}) ORDER BY RANDOM()",
        placeholders
    );
    let mut query = sqlx::query_as::<_, Question>(&sql);
    for id in &question_ids {
        query = query.bind(id);
    }
    query.fetch_all(&*pool).await.map_err(|e| format!("组卷失败: {}", e))
}

// ── 题库命令 ──────────────────────────────────────────────

#[tauri::command]
async fn get_random_question(
    tag: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Question, String> {
    let query_tag = format!("%{}%", tag);
    let question = sqlx::query_as::<_, Question>(
        "SELECT * FROM questions WHERE tags LIKE ? ORDER BY RANDOM() LIMIT 1",
    )
    .bind(query_tag)
    .fetch_optional(&*pool)
    .await
    .map_err(|e| format!("数据库查询失败: {}", e))?;

    match question {
        Some(q) => Ok(q),
        None => Err(format!("题库中暂时没有 [{}] 相关的题目。", tag)),
    }
}

#[tauri::command]
async fn generate_interview(
    tags: Vec<String>,
    count: u32,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<Question>, String> {
    let per_tag = count.max(1) as i64;
    let mut seen_ids = std::collections::HashSet::new();
    let mut result: Vec<Question> = Vec::new();

    for tag in tags {
        let query_tag = format!("%{}%", tag);
        let questions = sqlx::query_as::<_, Question>(
            "SELECT * FROM questions WHERE tags LIKE ? ORDER BY RANDOM() LIMIT ?",
        )
        .bind(query_tag)
        .bind(per_tag)
        .fetch_all(&*pool)
        .await
        .map_err(|e| format!("数据库查询失败: {}", e))?;

        for q in questions {
            if seen_ids.insert(q.id) {
                result.push(q);
            }
        }
    }

    if result.is_empty() {
        return Err("选中的考点下暂时没有题目，请重新选择或导入题库。".into());
    }
    Ok(result)
}

#[tauri::command]
async fn evaluate_answer(
    question_id: i32,
    user_answer: String,
    pool: tauri::State<'_, SqlitePool>,
    config: tauri::State<'_, Mutex<AppConfig>>,
) -> Result<EvaluateResponse, String> {
    let q = sqlx::query_as::<_, Question>(
        "SELECT * FROM questions WHERE id = ?",
    )
    .bind(question_id)
    .fetch_one(&*pool)
    .await
    .map_err(|e| format!("查询题目失败: {}", e))?;

    match q.question_type.as_str() {
        "SINGLE" => {
            let is_correct = user_answer.trim().eq_ignore_ascii_case(q.standard_answer.trim());
            let ai_comment = if is_correct {
                "✅ 回答正确！".to_string()
            } else {
                format!(
                    "❌ 回答有误。你选择了【{}】，正确答案是【{}】。",
                    user_answer.trim(),
                    q.standard_answer.trim()
                )
            };
            Ok(EvaluateResponse {
                standard_answer: q.standard_answer,
                explanation: q.explanation,
                is_correct: Some(is_correct),
                ai_comment,
                score: if is_correct { 100 } else { 0 },
            })
        }

        "MULTI" => {
            let normalize = |s: &str| -> Vec<char> {
                let mut v: Vec<char> = s
                    .split(|c: char| !c.is_ascii_alphabetic())
                    .flat_map(|seg| seg.chars())
                    .filter(|c| c.is_ascii_uppercase())
                    .collect();
                v.sort();
                v.dedup();
                v
            };
            let user_set = normalize(user_answer.trim());
            let std_set = normalize(q.standard_answer.trim());
            let is_correct = user_set == std_set;

            let ai_comment = if is_correct {
                "✅ 回答正确！".to_string()
            } else {
                let user_str: String = user_set.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", ");
                let std_str: String = std_set.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", ");
                format!("❌ 回答有误。你选择了【{}】，正确答案是【{}】。", user_str, std_str)
            };

            Ok(EvaluateResponse {
                standard_answer: q.standard_answer,
                explanation: q.explanation,
                is_correct: Some(is_correct),
                ai_comment,
                score: if is_correct { 100 } else { 0 },
            })
        }

        "ESSAY" | _ => {
            let (api_url, api_key) = {
                let cfg = config.lock().map_err(|e| e.to_string())?;
                (cfg.api_url.clone(), cfg.api_key.clone())
            };

            let (score, ai_comment) = llm_client::evaluate_essay_answer(
                &api_url,
                &api_key,
                &q.content,
                &q.standard_answer,
                &user_answer,
            )
            .await?;

            Ok(EvaluateResponse {
                standard_answer: q.standard_answer,
                explanation: q.explanation,
                is_correct: None,
                ai_comment,
                score,
            })
        }
    }
}

#[tauri::command]
async fn import_questions_from_file(
    file_path: String,
    pool: tauri::State<'_, SqlitePool>,
    config: tauri::State<'_, Mutex<AppConfig>>,
    app: tauri::AppHandle,
) -> Result<ImportResult, String> {
    let content = tokio::fs::read_to_string(&file_path)
        .await
        .map_err(|e| format!("文件读取失败: {}", e))?;
    let import_list: Vec<ImportQuestion> = serde_json::from_str(&content)
        .map_err(|e| format!("JSON 格式不正确: {}", e))?;
    let total = import_list.len();
    if total == 0 {
        return Err("文件内无题目".into());
    }

    let (api_url, api_key) = {
        let cfg = config.lock().map_err(|e| e.to_string())?;
        (cfg.api_url.clone(), cfg.api_key.clone())
    };

    let pool_clone = (*pool).clone();
    tokio::spawn(async move {
        let mut ai_count = 0;

        for (i, item) in import_list.into_iter().enumerate() {
            let current_idx = i + 1;

            let _ = app.emit("import-status", ImportProgress {
                current: current_idx,
                total,
                message: format!("正在处理: {:.30}...", item.content),
                is_finished: false,
            });

            let needs_ai = item.standard_answer.as_deref().unwrap_or("").trim().is_empty()
                || item.explanation.as_deref().unwrap_or("").trim().is_empty()
                || item.tags.trim().is_empty();

            let (ans, exp, tag) = if needs_ai {
                let options_text = item.options.as_ref().map(|o| o.join(", "));
                match llm_client::generate_answer_and_explanation(
                    &api_url,
                    &api_key,
                    &item.question_type,
                    &item.content,
                    options_text.as_deref(),
                )
                .await
                {
                    Ok((a, e, t)) => {
                        ai_count += 1;
                        let final_tag = if item.tags.trim().is_empty() { t } else { item.tags.clone() };
                        (a, e, final_tag)
                    }
                    Err(e) => {
                        eprintln!("⚠️ 第 {} 题 AI 补全失败: {}", current_idx, e);
                        (
                            item.standard_answer.unwrap_or_default(),
                            item.explanation.unwrap_or_default(),
                            item.tags.clone(),
                        )
                    }
                }
            } else {
                (
                    item.standard_answer.unwrap_or_default(),
                    item.explanation.unwrap_or_default(),
                    item.tags.clone(),
                )
            };

            let options_json = item.options.map(|o| serde_json::to_string(&o).unwrap_or_default());

            let res = sqlx::query(
                "INSERT INTO questions
                    (question_type, content, options, tags, difficulty, standard_answer, explanation)
                 VALUES (?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(content) DO UPDATE SET
                    standard_answer = excluded.standard_answer,
                    explanation = excluded.explanation,
                    tags = excluded.tags",
            )
            .bind(&item.question_type)
            .bind(&item.content)
            .bind(&options_json)
            .bind(&tag)
            .bind(item.difficulty)
            .bind(&ans)
            .bind(&exp)
            .execute(&pool_clone)
            .await;

            if let Err(e) = res {
                eprintln!("❌ 第 {} 题入库失败: {}", current_idx, e);
            }
        }

        let _ = app.emit("import-status", ImportProgress {
            current: total,
            total,
            message: format!("🎉 导入完成！AI 补全/规范化分类了 {} 道题目。", ai_count),
            is_finished: true,
        });
    });

    Ok(ImportResult {
        total,
        ai_generated: 0,
        message: format!("已启动后台导入，共 {} 题，正在进行 AI 语义分类...", total),
    })
}

#[tauri::command]
async fn get_all_tags(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<String>, String> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT tags FROM questions")
        .fetch_all(&*pool)
        .await
        .map_err(|e| format!("查询标签失败: {}", e))?;

    let mut tag_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (tags_str,) in rows {
        for tag in tags_str.split(',') {
            let t = tag.trim().to_string();
            if !t.is_empty() {
                tag_set.insert(t);
            }
        }
    }

    let mut tags: Vec<String> = tag_set.into_iter().collect();
    tags.sort();
    Ok(tags)
}

#[tauri::command]
async fn get_tag_counts(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<std::collections::HashMap<String, usize>, String> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT tags FROM questions")
        .fetch_all(&*pool)
        .await
        .map_err(|e| format!("查询标签失败: {}", e))?;

    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (tags_str,) in rows {
        for tag in tags_str.split(',') {
            let t = tag.trim().to_string();
            if !t.is_empty() {
                *counts.entry(t).or_insert(0) += 1;
            }
        }
    }
    Ok(counts)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 加载配置文件（API Key 等）
            let config_dir = app
                .path()
                .app_config_dir()
                .expect("无法获取应用配置目录");
            let cfg = AppConfig::load(&config_dir);
            app.manage(Mutex::new(cfg));
            app.manage(ConfigDir(config_dir));

            // 初始化数据库
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match db::init_db().await {
                    Ok(pool) => {
                        handle.manage(pool);
                        println!("数据库连接池挂载成功！");
                    }
                    Err(e) => eprintln!("数据库初始化失败: {}", e),
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_random_question,
            generate_interview,
            generate_interview_from_ids,
            evaluate_answer,
            import_questions_from_file,
            get_all_tags,
            get_tag_counts,
            get_api_config,
            set_api_config,
            save_training_session,
            get_wrong_questions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
