use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};
use std::path::PathBuf;

use crate::models::Topic;

const DEFAULT_TOPICS: &[(&str, &str)] = &[
    ("Java", "Java language and JVM interview topic"),
    ("Rust", "Rust language interview topic"),
    ("操作系统", "Operating system interview topic"),
    ("计算机网络", "Computer networking interview topic"),
    ("数据库", "Database interview topic"),
    ("数据结构", "Data structure interview topic"),
    ("其他", "General interview topic"),
];

pub async fn list_topics(pool: &SqlitePool) -> Result<Vec<Topic>, sqlx::Error> {
    sqlx::query_as::<_, Topic>(
        "SELECT id, name, description, created_at
         FROM topics
         ORDER BY lower(name) ASC",
    )
    .fetch_all(pool)
    .await
}

pub async fn create_topic(
    pool: &SqlitePool,
    name: &str,
    description: &str,
) -> Result<Topic, String> {
    let name = name.trim();
    let description = description.trim();
    if name.is_empty() {
        return Err("考点名称不能为空".into());
    }
    if name.contains(',') {
        return Err("考点名称不能包含英文逗号".into());
    }

    sqlx::query(
        "INSERT INTO topics (name, description)
         VALUES (?, ?)
         ON CONFLICT(name) DO UPDATE SET
            description = CASE
                WHEN excluded.description = '' THEN topics.description
                ELSE excluded.description
            END",
    )
    .bind(name)
    .bind(description)
    .execute(pool)
    .await
    .map_err(|e| format!("保存考点失败: {}", e))?;

    sqlx::query_as::<_, Topic>(
        "SELECT id, name, description, created_at
         FROM topics
         WHERE name = ?",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("读取考点失败: {}", e))
}

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

async fn seed_default_topics(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    for (name, description) in DEFAULT_TOPICS {
        sqlx::query(
            "INSERT OR IGNORE INTO topics (name, description)
             VALUES (?, ?)",
        )
        .bind(name)
        .bind(description)
        .execute(pool)
        .await?;
    }
    Ok(())
}

async fn seed_topics_from_questions(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT tags FROM questions")
        .fetch_all(pool)
        .await?;

    for (tags,) in rows {
        for tag in tags.split(',') {
            let name = tag.trim();
            if name.is_empty() {
                continue;
            }
            sqlx::query(
                "INSERT OR IGNORE INTO topics (name, description)
                 VALUES (?, '')",
            )
            .bind(name)
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}

pub async fn init_db(db_path: PathBuf) -> Result<SqlitePool, sqlx::Error> {
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let options = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;  
  
    // DDL：新增 explanation 字段  
    sqlx::query(  
        "CREATE TABLE IF NOT EXISTS questions (  
            id              INTEGER PRIMARY KEY AUTOINCREMENT,  
            question_type   TEXT    NOT NULL,  
            content         TEXT    NOT NULL UNIQUE,  
            options         TEXT,  
            tags            TEXT    NOT NULL,  
            difficulty      INTEGER NOT NULL,  
            standard_answer TEXT    NOT NULL DEFAULT '',  
            explanation     TEXT    NOT NULL DEFAULT ''  
        );"  
    )  
    .execute(&pool)  
    .await?;  
  
    let _ = sqlx::query("ALTER TABLE questions ADD COLUMN explanation TEXT NOT NULL DEFAULT ''")
        .execute(&pool)
        .await;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS training_sessions (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at    TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
            total_count   INTEGER NOT NULL,
            correct_count INTEGER NOT NULL,
            average_score REAL    NOT NULL,
            skipped_count INTEGER NOT NULL DEFAULT 0,
            tags          TEXT    NOT NULL DEFAULT ''
        );"
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS training_records (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id     INTEGER NOT NULL REFERENCES training_sessions(id) ON DELETE CASCADE,
            question_id    INTEGER NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
            user_answer    TEXT    NOT NULL DEFAULT '',
            score          INTEGER NOT NULL DEFAULT 0,
            is_correct     INTEGER,
            skipped        INTEGER NOT NULL DEFAULT 0,
            manually_added INTEGER NOT NULL DEFAULT 0,
            time_spent     INTEGER NOT NULL DEFAULT 0,
            created_at     TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
        );"
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS topics (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT    NOT NULL UNIQUE,
            description TEXT    NOT NULL DEFAULT '',
            created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
        );"
    )
    .execute(&pool)
    .await?;
    seed_default_topics(&pool).await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS mock_interviews (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at    TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
            ended_at      TEXT,
            tags          TEXT    NOT NULL DEFAULT '',
            question_count INTEGER NOT NULL,
            average_score REAL    NOT NULL DEFAULT 0,
            summary       TEXT    NOT NULL DEFAULT '',
            status        TEXT    NOT NULL DEFAULT 'active'
        );"
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS mock_interview_turns (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            interview_id     INTEGER NOT NULL REFERENCES mock_interviews(id) ON DELETE CASCADE,
            question_id      INTEGER NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
            question_content TEXT    NOT NULL,
            user_answer      TEXT    NOT NULL DEFAULT '',
            ai_comment       TEXT    NOT NULL DEFAULT '',
            follow_up        TEXT    NOT NULL DEFAULT '',
            follow_up_answer TEXT    NOT NULL DEFAULT '',
            score            INTEGER NOT NULL DEFAULT 0,
            created_at       TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
        );"
    )
    .execute(&pool)
    .await?;

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM questions")  
        .fetch_one(&pool)  
        .await?;  
  
    if count.0 == 0 {  
        sqlx::query(  
            "INSERT INTO questions   
                (question_type, content, options, tags, difficulty, standard_answer, explanation)   
             VALUES  
                ('ESSAY',   
                 '请简述进程与线程的区别？',   
                 NULL,   
                 '操作系统',   
                 3,   
                 '进程是资源分配的最小单位，线程是CPU调度的最小单位。进程拥有独立的地址空间，线程共享进程的地址空间。进程切换开销大，线程切换开销小。',   
                 '这是操作系统的经典考题。核心区分点：①资源独立性：进程独立，线程共享；②切换开销：进程 > 线程；③通信方式：进程用IPC，线程直接读写共享内存；④健壮性：一个进程崩溃不影响其他进程，但线程崩溃会导致整个进程崩溃。'  
                ),  
                ('SINGLE',   
                 '在 Rust 中，哪个关键字用于声明不可变变量？',   
                 '[\"A. let\", \"B. mut\", \"C. static\", \"D. const\"]',   
                 'Rust',   
                 1,   
                 'A',   
                 'Rust 中用 let 声明变量，默认不可变。要声明可变变量需要 let mut。注意区分：let 是运行时绑定，const 是编译期常量，static 是静态变量有固定内存地址。'  
                ),  
                ('SINGLE',   
                 'OSI 七层模型中，负责路径选择的是哪一层？',   
                 '[\"A. 物理层\", \"B. 数据链路层\", \"C. 网络层\", \"D. 传输层\"]',   
                 '计算机网络',   
                 2,   
                 'C',   
                 '网络层（第3层）负责逻辑寻址和路由选择，核心设备是路由器。记忆口诀：物数网传会表应。路由器工作在网络层，交换机工作在数据链路层，集线器工作在物理层。'  
                );"  
        )  
        .execute(&pool)  
        .await?;  
        println!("🚀 种子数据注入成功（含解析）！");  
    }  
    seed_topics_from_questions(&pool).await?;
    Ok(pool)  
}  

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_db_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("forgerust-{}-{}.db", name, nonce))
    }

    #[tokio::test]
    async fn init_db_seeds_default_topics_and_allows_creating_new_topic() {
        let db_path = test_db_path("topics");
        let pool = init_db(db_path.clone()).await.unwrap();

        let topics = list_topics(&pool).await.unwrap();
        assert!(topics.iter().any(|topic| topic.name == "Rust"));
        assert!(topics.iter().any(|topic| topic.name == "Java"));

        create_topic(&pool, "Linux", "Operating system interview topic")
            .await
            .unwrap();

        let topics = list_topics(&pool).await.unwrap();
        let linux = topics.iter().find(|topic| topic.name == "Linux").unwrap();
        assert_eq!(linux.description, "Operating system interview topic");

        pool.close().await;
        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn init_db_creates_mock_interview_tables() {
        let db_path = test_db_path("mock-interview");
        let pool = init_db(db_path.clone()).await.unwrap();

        let interview_table: String = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'mock_interviews'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let turn_table: String = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'mock_interview_turns'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(interview_table, "mock_interviews");
        assert_eq!(turn_table, "mock_interview_turns");

        pool.close().await;
        let _ = std::fs::remove_file(db_path);
    }

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
            &pool, id as i32, "ESSAY", "什么是所有权与借用？", None, "Rust", 3,
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
}
