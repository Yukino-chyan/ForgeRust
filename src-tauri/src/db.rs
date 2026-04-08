use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::str::FromStr;

// 初始化数据库，返回一个连接池
pub async fn init_db() -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str("sqlite://forgerust.db")?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    // 1. 更新 DDL：增加了类型、选项和标准答案字段
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS questions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            question_type TEXT NOT NULL,
            content TEXT NOT NULL,
            options TEXT, 
            tags TEXT NOT NULL,
            difficulty INTEGER NOT NULL,
            standard_answer TEXT NOT NULL
        );"
    )
    .execute(&pool)
    .await?;

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM questions")
        .fetch_one(&pool)
        .await?;

    if count.0 == 0 {
        // 2. 插入更具代表性的种子数据
        sqlx::query(
            "INSERT INTO questions (question_type, content, options, tags, difficulty, standard_answer) VALUES
            ('ESSAY', '请简述进程与线程的区别？', NULL, '操作系统', 3, '进程是资源分配的最小单位，线程是CPU调度的最小单位。'),
            ('SINGLE', '在 Rust 中，哪个关键字用于声明不可变变量？', '[\"A. let\", \"B. mut\", \"C. static\", \"D. const\"]', 'Rust基础', 1, 'A'),
            ('SINGLE', 'OSI 七层模型中，负责路径选择的是哪一层？', '[\"A. 物理层\", \"B. 数据链路层\", \"C. 网络层\", \"D. 传输层\"]', '计算机网络', 2, 'C');"
        )
        .execute(&pool)
        .await?;
        println!("🚀 数据库已更新，多题型种子数据注入成功！");
    }

    Ok(pool)
}