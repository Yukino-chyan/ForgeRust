// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod db;
mod models;

use std::fs;
use crate::models::{MockAiResponse, Question};
use sqlx::SqlitePool;
use tauri::Manager;
use tokio::time::{sleep, Duration};

// 出题函数
#[tauri::command]
async fn get_mock_question(
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

    // 处理查不到题目的情况
    match question {
        Some(q) => Ok(q),
        None => Err(format!(
            "题库中暂时没有关于 [{}] 的题目，请换个标签试试。",
            tag
        )),
    }
}

// 开展面试，根据用户选的标签生成一套试卷（多题版）
#[tauri::command]
async fn generate_interview(
    tags: Vec<String>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<Question>, String> {
    let mut interview_questions: Vec<Question> = Vec::new();

    // 遍历用户选中的每一个标签，去数据库里各抽 2 道题（可自行调整数量）
    for tag in tags {
        let query_tag = format!("%{}%", tag);
        let mut questions = sqlx::query_as::<_, Question>(
            "SELECT * FROM questions WHERE tags LIKE ? ORDER BY RANDOM() LIMIT 2"
        )
        .bind(query_tag)
        .fetch_all(&*pool)
        .await
        .map_err(|e| format!("数据库查询失败: {}", e))?;
        interview_questions.append(&mut questions);
    }
    if interview_questions.is_empty() {
        return Err("选中的考点下暂时没有题目，请重新选择或导入题库。".into());
    }
    Ok(interview_questions)
}

// 面试官打分和点评的逻辑
#[tauri::command]
async fn mock_evaluate_answer(answer: String) -> Result<MockAiResponse, String> {
    println!("接收到用户回答: {}", answer);
    sleep(Duration::from_secs(2)).await;
    Ok(MockAiResponse {
        score: 85,
        comment: "你的回答逻辑非常清晰，准确提到了 Rust 的所有权机制。但在生命周期（Lifetime）的解释上还可以更深入一些。".into(),
        next_topic_suggestion: "要不要聊聊智能指针（Smart Pointers）？".into(),
    })
}

// 文件导入函数
#[tauri::command]
async fn import_questions_from_file(
    file_path: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<String, String> {
    
    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("读取文件失败: {}", e))?;

    println!("读取成功。正在调用 AI 清洗...");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await; 

    let insert_result = sqlx::query(
        "INSERT INTO questions (content, tags, difficulty) VALUES
        ('【导入测试】操作系统的内存碎片是如何产生的？有哪些解决策略？', '操作系统', 3),
        ('【导入测试】详细对比一下 HTTP/1.1、HTTP/2 和 HTTP/3 的核心区别。', '计算机网络', 4),
        ('【导入测试】请解释一下 Spring Boot 的自动装配（Auto-Configuration）原理。', 'Java后端', 4);"
    )
    .execute(&*pool)
    .await
    .map_err(|e| format!("写入数据库失败: {}", e))?;

    Ok(format!("成功！从文件中提取并导入了 {} 道题目。", insert_result.rows_affected()))
}

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
                        println!("🎉 数据库连接池已成功挂载到全局状态！");
                    }
                    Err(e) => eprintln!("❌ 数据库初始化失败: {}", e),
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            mock_evaluate_answer,
            get_mock_question,
            import_questions_from_file,
            generate_interview
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
