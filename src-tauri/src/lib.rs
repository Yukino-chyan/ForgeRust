// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod models;

use crate::models::{Question, MockAiResponse};
use tokio::time::{sleep, Duration};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

// 这是一个模拟的出题函数
#[tauri::command]
fn get_mock_question(tag: String) -> Question {
    // 模拟从数据库筛选逻辑
    match tag.as_str() {
        "操作系统" => Question {
            id: 1,
            content: "请简述进程与线程的区别，以及 Rust 是如何通过所有权机制保证线程安全的？".into(),
            tags: "操作系统,并发".into(),
            difficulty: 3,
        },
        "计算机网络" => Question {
            id: 2,
            content: "请详细描述 TCP 三次握手的过程，并解释为什么要进行第三次握手？".into(),
            tags: "网络".into(),
            difficulty: 2,
        },
        "Java后端" => Question {
            id: 3,
            content: "谈谈 JVM 的垃圾回收机制中，G1 收集器与 CMS 收集器的主要区别是什么？".into(),
            tags: "Java,JVM".into(),
            difficulty: 4,
        },
        _ => Question {
            id: 0,
            content: "准备好了吗？点击下方标签选择一个面试方向开始。".into(),
            tags: "通用".into(),
            difficulty: 1,
        },
    }
}

// 模拟面试官打分和点评的逻辑
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
        .invoke_handler(tauri::generate_handler![
            greet, 
            mock_evaluate_answer,
            get_mock_question
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}