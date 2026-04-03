use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::str::FromStr;

// 初始化数据库，返回一个连接池
pub async fn init_db() -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str("sqlite://forgerust.db")?.create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS questions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content TEXT NOT NULL,
            tags TEXT NOT NULL,
            difficulty INTEGER NOT NULL
        );",
    )
    .execute(&pool)
    .await?;

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM questions")
        .fetch_one(&pool)
        .await?;

    if count.0 == 0 {
        sqlx::query(
            "INSERT INTO questions (content, tags, difficulty) VALUES
            ('请简述进程与线程的区别，以及 Rust 是如何保证线程安全的？', '操作系统', 3),
            ('请解释 TCP 三次握手的过程。', '计算机网络', 2),
            ('谈谈 JVM 的垃圾回收机制中，G1 收集器与 CMS 收集器的主要区别？', 'Java后端', 4);",
        )
        .execute(&pool)
        .await?;
        println!("🚀 数据库为空，已自动插入初始面试题！");
    } else {
        println!("✅ 数据库已连接，现有题目数量: {}", count.0);
    }

    Ok(pool)
}
