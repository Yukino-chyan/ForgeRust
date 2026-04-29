use sqlx::{  
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},  
    SqlitePool,  
};  
use std::str::FromStr;  
  
pub async fn init_db() -> Result<SqlitePool, sqlx::Error> {  
    let options = SqliteConnectOptions::from_str("sqlite://forgerust.db")?  
        .create_if_missing(true);  
  
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
    Ok(pool)  
}  
