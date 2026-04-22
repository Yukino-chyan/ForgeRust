mod db;  
mod models;  
mod llm_client;  
  
use crate::models::{EvaluateResponse, ImportQuestion, ImportResult, Question, ImportProgress}; // 👈 加上 ImportProgress
use sqlx::SqlitePool;  
use tauri::Manager;
use tauri::Emitter;
   
// 按标签随机抽一题
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
  
// 按多个标签组卷
#[tauri::command]
async fn generate_interview(
    tags: Vec<String>,
    count: u32,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<Question>, String> {
    let per_tag = count.max(1) as i64;
    let mut result: Vec<Question> = Vec::new();

    for tag in tags {
        let query_tag = format!("%{}%", tag);
        let mut questions = sqlx::query_as::<_, Question>(
            "SELECT * FROM questions WHERE tags LIKE ? ORDER BY RANDOM() LIMIT ?",
        )
        .bind(query_tag)
        .bind(per_tag)
        .fetch_all(&*pool)
        .await
        .map_err(|e| format!("数据库查询失败: {}", e))?;
        result.append(&mut questions);
    }

    if result.is_empty() {
        return Err("选中的考点下暂时没有题目，请重新选择或导入题库。".into());
    }
    Ok(result)
}  
   
// 评分
#[tauri::command]  
async fn evaluate_answer(  
    question_id: i32,  
    user_answer: String,  
    pool: tauri::State<'_, SqlitePool>,  
) -> Result<EvaluateResponse, String> {  
  
    // 从数据库取出完整题目信息  
    let q = sqlx::query_as::<_, Question>(  
        "SELECT * FROM questions WHERE id = ?",  
    )  
    .bind(question_id)  
    .fetch_one(&*pool)  
    .await  
    .map_err(|e| format!("查询题目失败: {}", e))?;  
  
    // 策略路由  
    match q.question_type.as_str() {  
  
        "SINGLE" => {
            let is_correct = user_answer
                .trim()
                .eq_ignore_ascii_case(q.standard_answer.trim());

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
            // 将用户答案和标准答案都规范化为排序后的字母集合再比较
            // 兼容 "A,B" 和 "AB" 两种格式
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
            let std_set  = normalize(q.standard_answer.trim());
            let is_correct = user_set == std_set;

            let ai_comment = if is_correct {
                "✅ 回答正确！".to_string()
            } else {
                let user_str: String = user_set.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", ");
                let std_str:  String = std_set.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", ");
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

        // ── 简答题：调用 AI 实时点评 ─────────────────────────
        "ESSAY" | _ => {  
            let (score, ai_comment) = llm_client::evaluate_essay_answer(  
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
   
// 导入题库  
#[tauri::command]
async fn import_questions_from_file(
    file_path: String,
    pool: tauri::State<'_, SqlitePool>,
    app: tauri::AppHandle,
) -> Result<ImportResult, String> {
    let content = tokio::fs::read_to_string(&file_path)
        .await
        .map_err(|e| format!("文件读取失败: {}", e))?;
    let import_list: Vec<ImportQuestion> = serde_json::from_str(&content)
        .map_err(|e| format!("JSON 格式不正确: {}", e))?;
    let total = import_list.len();
    if total == 0 { return Err("文件内无题目".into()); }
    let pool_clone = (*pool).clone();
    tokio::spawn(async move {
        let mut ai_count = 0;
        
        for (i, item) in import_list.into_iter().enumerate() {
            let current_idx = i + 1;
            
            // 1. 发送进度
            let _ = app.emit("import-status", ImportProgress {
                current: current_idx,
                total,
                message: format!("正在处理: {:.10}...", item.content),
                is_finished: false,
            });

            // 2. 判定是否需要 AI (答案为空、解析为空 或 标签为空)
            let needs_ai = item.standard_answer.as_deref().unwrap_or("").trim().is_empty() 
                        || item.explanation.as_deref().unwrap_or("").trim().is_empty()
                        || item.tags.trim().is_empty();

            let (ans, exp, tag) = if needs_ai {
                let options_text = item.options.as_ref().map(|o| o.join(", "));
                match llm_client::generate_answer_and_explanation(
                    &item.question_type, 
                    &item.content, 
                    options_text.as_deref()
                ).await {
                    Ok((a, e, t)) => {
                        ai_count += 1;
                        // 如果原文件有标签则用原文件的，否则用 AI 归一化后的标签
                        let final_tag = if item.tags.trim().is_empty() { t } else { item.tags.clone() };
                        (a, e, final_tag)
                    },
                    Err(e) => {
                        eprintln!("⚠️ 第 {} 题 AI 补全失败: {}", current_idx, e);
                        (item.standard_answer.unwrap_or_default(), item.explanation.unwrap_or_default(), item.tags.clone())
                    }
                }
            } else {
                (item.standard_answer.unwrap_or_default(), item.explanation.unwrap_or_default(), item.tags.clone())
            };

            // 3. 写入数据库 (UPSERT 逻辑：内容重复则更新答案、解析和标签)
            let options_json = item.options.map(|o| serde_json::to_string(&o).unwrap_or_default());
            
            let res = sqlx::query(
                "INSERT INTO questions 
                    (question_type, content, options, tags, difficulty, standard_answer, explanation) 
                 VALUES (?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(content) DO UPDATE SET 
                    standard_answer = excluded.standard_answer,
                    explanation = excluded.explanation,
                    tags = excluded.tags"
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

        // 4. 发送完成事件
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
  
// 命令5：获取所有可用标签（动态从数据库读取）  
#[tauri::command]  
async fn get_all_tags(  
    pool: tauri::State<'_, SqlitePool>,  
) -> Result<Vec<String>, String> {  
    // 查出所有 tags 字段，然后在 Rust 侧分割去重  
    let rows: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT tags FROM questions")  
        .fetch_all(&*pool)  
        .await  
        .map_err(|e| format!("查询标签失败: {}", e))?;  
  
    // tags 字段可能是 "Java后端,计算机网络" 这种多标签格式，需要拆分  
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
    tags.sort(); // 字母序排列  
    Ok(tags)  
}  
  
// App 启动入口  
#[cfg_attr(mobile, tauri::mobile_entry_point)]  
pub fn run() {  
    tauri::Builder::default()  
        .plugin(tauri_plugin_dialog::init())  
        .plugin(tauri_plugin_opener::init())  
        .setup(|app| {  
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
            evaluate_answer,           // 替换原来的 mock_evaluate_answer  
            import_questions_from_file,  
            get_all_tags,              // 新增：动态读取标签  
        ])  
        .run(tauri::generate_context!())  
        .expect("error while running tauri application");  
}  
