mod db;  
mod models;  
mod llm_client;  
  
use crate::models::{EvaluateResponse, ImportQuestion, ImportResult, Question};  
use sqlx::SqlitePool;  
use tauri::Manager;  
   
// 命令1：按标签随机抽一题（题库训练用）   
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
  
// 命令2：按多个标签组卷（模拟面试用，暂时保留）  
#[tauri::command]  
async fn generate_interview(  
    tags: Vec<String>,  
    pool: tauri::State<'_, SqlitePool>,  
) -> Result<Vec<Question>, String> {  
    let mut result: Vec<Question> = Vec::new();  
  
    for tag in tags {  
        let query_tag = format!("%{}%", tag);  
        let mut questions = sqlx::query_as::<_, Question>(  
            "SELECT * FROM questions WHERE tags LIKE ? ORDER BY RANDOM() LIMIT 2",  
        )  
        .bind(query_tag)  
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
   
// 命令3：评分（核心改造）  
#[tauri::command]  
async fn evaluate_answer(  
    question_id: i32,  
    user_answer: String,  
    pool: tauri::State<'_, SqlitePool>,  
) -> Result<EvaluateResponse, String> {  
  
    // 1. 从数据库取出完整题目信息  
    let q = sqlx::query_as::<_, Question>(  
        "SELECT * FROM questions WHERE id = ?",  
    )  
    .bind(question_id)  
    .fetch_one(&*pool)  
    .await  
    .map_err(|e| format!("查询题目失败: {}", e))?;  
  
    // 2. 策略路由  
    match q.question_type.as_str() {  
  
        "SINGLE" | "MULTI" => {  
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
                is_correct: None, // 简答题不判断对错，只给分  
                ai_comment,  
                score,  
            })  
        }  
    }  
}  
   
// 命令4：导入题库（真实解析 + AI 后台补全）   
#[tauri::command]  
async fn import_questions_from_file(  
    file_path: String,  
    pool: tauri::State<'_, SqlitePool>,  
) -> Result<ImportResult, String> {  
  
    // 1. 读取并解析 JSON 文件  
    let content = std::fs::read_to_string(&file_path)  
        .map_err(|e| format!("读取文件失败: {}", e))?;  
  
    let import_list: Vec<ImportQuestion> = serde_json::from_str(&content)  
        .map_err(|e| format!(  
            "JSON 解析失败，请检查文件格式是否正确: {}", e  
        ))?;  
  
    let total = import_list.len();  
    if total == 0 {  
        return Err("文件中没有找到任何题目，请检查文件内容。".into());  
    }  
  
    let mut ai_generated_count = 0usize;  
    // clone pool 出来，让后台任务能独立持有  
    let pool_clone = (*pool).clone();  
  
    // 2. 启动后台任务处理（不阻塞前端）  
    tokio::spawn(async move {  
        println!("🚀 后台导入任务启动，共 {} 道题目...", total);  
  
        for item in import_list {  
            // 把 options Vec 转成 JSON 字符串（入库格式）  
            let options_str: Option<String> = item.options.as_ref().map(|opts| {  
                serde_json::to_string(opts).unwrap_or_default()  
            });  
  
            // 检查是否需要 AI 补全  
            let needs_ai = item.standard_answer  
                .as_deref()  
                .map(|s| s.trim().is_empty())  
                .unwrap_or(true)  
                || item.explanation  
                    .as_deref()  
                    .map(|s| s.trim().is_empty())  
                    .unwrap_or(true);  
  
            let (final_answer, final_explanation) = if needs_ai {  
                println!("🤖 AI 补全中：{}", &item.content);  
  
                // 格式化选项用于 Prompt  
                let options_for_prompt = item.options.as_ref().map(|opts| opts.join(", "));  
  
                match llm_client::generate_answer_and_explanation(  
                    &item.question_type,  
                    &item.content,  
                    options_for_prompt.as_deref(),  
                )  
                .await  
                {  
                    Ok((answer, explanation)) => {  
                        ai_generated_count += 1;  
                        (answer, explanation)  
                    }  
                    Err(e) => {  
                        // AI 补全失败：记录日志，用空值占位，不中断整个导入  
                        eprintln!("⚠️ AI 补全失败（{}）: {}", &item.content, e);  
                        (  
                            item.standard_answer.unwrap_or_default(),  
                            item.explanation.unwrap_or_default(),  
                        )  
                    }  
                }  
            } else {  
                // 不需要AI补全，直接用文件里的数据  
                (  
                    item.standard_answer.unwrap_or_default(),  
                    item.explanation.unwrap_or_default(),  
                )  
            };  
  
            // 插入数据库  
            let insert_result = sqlx::query(  
                "INSERT INTO questions   
                    (question_type, content, options, tags, difficulty, standard_answer, explanation)  
                 VALUES (?, ?, ?, ?, ?, ?, ?)",  
            )  
            .bind(&item.question_type)  
            .bind(&item.content)  
            .bind(&options_str)  
            .bind(&item.tags)  
            .bind(item.difficulty)  
            .bind(&final_answer)  
            .bind(&final_explanation)  
            .execute(&pool_clone)  
            .await;  
  
            match insert_result {  
                Ok(_)  => println!("✅ 入库成功：{}", &item.content),  
                Err(e) => eprintln!("❌ 入库失败：{}，原因：{}", &item.content, e),  
            }  
        }  
  
        println!(  
            "🎉 导入任务完成！共 {} 题，AI 补全了 {} 题的答案和解析。",  
            total, ai_generated_count  
        );  
    });  
  
    // 3. 立即返回给前端（后台继续跑）  
    Ok(ImportResult {  
        total,  
        ai_generated: 0, // 后台异步，此时还不知道最终数量  
        message: format!(  
            "已接收 {} 道题目，正在后台解析入库中，AI 将自动补全缺失的答案和解析，请稍候...",  
            total  
        ),  
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
                        println!("🎉 数据库连接池挂载成功！");  
                    }  
                    Err(e) => eprintln!("❌ 数据库初始化失败: {}", e),  
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
