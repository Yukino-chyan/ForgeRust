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
// 注意：参数增加了 question_id，并且引入了 SqlitePool
#[tauri::command]
async fn mock_evaluate_answer(
    question_id: i32,
    user_answer: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<MockAiResponse, String> {
    
    // 1. 去数据库查出这道题到底是什么类型，标准答案是什么
    let q = sqlx::query_as::<_, Question>("SELECT * FROM questions WHERE id = ?")
        .bind(question_id)
        .fetch_one(&*pool)
        .await
        .map_err(|e| format!("查询题目信息失败: {}", e))?;

    // 2. 策略路由分发 (Strategy Pattern)
    if q.question_type == "SINGLE" || q.question_type == "MULTI" {
        // 【策略 A：本地极速判卷】零成本，零延迟
        let is_correct = user_answer.trim().eq_ignore_ascii_case(q.standard_answer.trim());
        
        if is_correct {
            Ok(MockAiResponse {
                score: 100,
                comment: "✅ 回答正确！基础非常扎实。".into(),
                next_topic_suggestion: "继续保持这种敏锐度。".into(),
            })
        } else {
            Ok(MockAiResponse {
                score: 0,
                comment: format!("❌ 回答错误。正确答案是：{}", q.standard_answer),
                next_topic_suggestion: "建议回炉重造相关基础概念。".into(),
            })
        }
    } else {
        // 【策略 B：大模型 AI 判卷】(目前仍然用 sleep 模拟)
        println!("📝 收到简答题，正在调用 AI 模型分析... 参考答案: {}", q.standard_answer);
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        Ok(MockAiResponse {
            score: 85,
            comment: "逻辑比较清晰，但深度可以再加强。建议结合底层源码来解释。".into(),
            next_topic_suggestion: "是否需要我为你详细拆解一下标准答案的逻辑？".into(),
        })
    }
}

// 文件导入函数
// 替换原有的 import_questions_from_file 函数
#[tauri::command]
async fn import_questions_from_file(
    file_path: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<String, String> {
    
    // 1. 读取本地文件 (虽然现在还没真用上内容，但保留链路)
    let _content = std::fs::read_to_string(&file_path)
        .map_err(|e| format!("读取文件失败: {}", e))?;

    // 2. 模拟 AI 清洗时间
    println!("正在调用 AI 解析文件，提取新题型...");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await; 

    // 3. 🚨 重点：插入符合【新表结构】的题目！补充了 Java 后端的题目
    let insert_result = sqlx::query(
        "INSERT INTO questions (question_type, content, options, tags, difficulty, standard_answer) VALUES
        ('ESSAY', '【导入】请解释一下 Redis 为什么这么快？', NULL, 'Java后端', 3, '纯内存操作、单线程避免上下文切换、IO多路复用。'),
        ('SINGLE', '【导入】TCP 协议属于 OSI 模型的哪一层？', '[\"A. 应用层\", \"B. 传输层\", \"C. 网络层\", \"D. 数据链路层\"]', '计算机网络', 2, 'B'),
        ('SINGLE', '【导入】Java 中哪个关键字用于保证变量的内存可见性？', '[\"A. static\", \"B. final\", \"C. volatile\", \"D. synchronized\"]', 'Java后端', 3, 'C');"
    )
    .execute(&*pool)
    .await
    .map_err(|e| format!("写入数据库失败: {}", e))?;

    Ok(format!("成功！导入了 {} 道新格式的题目。", insert_result.rows_affected()))
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
