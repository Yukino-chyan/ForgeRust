// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod models;
mod db;

use crate::models::{Question, MockAiResponse};
use tokio::time::{sleep, Duration};
use tauri::Manager;
use sqlx::SqlitePool;

// 出题函数
#[tauri::command]
async fn get_mock_question(
    tag: String, 
    pool: tauri::State<'_, SqlitePool>
) -> Result<Question, String> {
    let query_tag = format!("%{}%", tag);
    let question = sqlx::query_as::<_, Question>(
        "SELECT * FROM questions WHERE tags LIKE ? ORDER BY RANDOM() LIMIT 1"
    )
    .bind(query_tag)
    .fetch_optional(&*pool)
    .await
    .map_err(|e| format!("数据库查询失败: {}", e))?;

    // 处理查不到题目的情况
    match question {
        Some(q) => Ok(q),
        None => Err(format!("题库中暂时没有关于 [{}] 的题目，请换个标签试试。", tag)),
    }
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
            get_mock_question
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}