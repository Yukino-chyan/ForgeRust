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
    ("数据库", "数据库面试考点"),
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

fn is_question_hash_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
        || matches!(
            ch as u32,
            0x3400..=0x4DBF
                | 0x4E00..=0x9FFF
                | 0xF900..=0xFAFF
                | 0x20000..=0x2A6DF
                | 0x2A700..=0x2B73F
                | 0x2B740..=0x2B81F
                | 0x2B820..=0x2CEAF
        )
}

pub fn normalize_question_content(content: &str) -> String {
    let mut normalized = String::new();
    for ch in content.chars() {
        for lower in ch.to_lowercase() {
            if is_question_hash_char(lower) {
                normalized.push(lower);
            }
        }
    }
    normalized
}

fn hash_normalized_content(normalized: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in normalized.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub fn question_content_hash(content: &str) -> String {
    hash_normalized_content(&normalize_question_content(content))
}

pub async fn find_question_id_by_content(
    pool: &SqlitePool,
    content: &str,
) -> Result<Option<i32>, String> {
    sqlx::query_scalar::<_, i32>("SELECT id FROM questions WHERE content = ?")
        .bind(content.trim())
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("查询已有题目失败: {}", e))
}

async fn find_duplicate_question_id(
    pool: &SqlitePool,
    content_hash: &str,
    exclude_id: Option<i32>,
) -> Result<Option<i32>, String> {
    match exclude_id {
        Some(id) => sqlx::query_scalar::<_, i32>(
            "SELECT id FROM questions WHERE content_hash = ? AND id <> ? ORDER BY id ASC LIMIT 1",
        )
        .bind(content_hash)
        .bind(id)
        .fetch_optional(pool)
        .await,
        None => {
            sqlx::query_scalar::<_, i32>(
                "SELECT id FROM questions WHERE content_hash = ? ORDER BY id ASC LIMIT 1",
            )
            .bind(content_hash)
            .fetch_optional(pool)
            .await
        }
    }
    .map_err(|e| format!("查询重复题失败: {}", e))
}

pub async fn prepare_question_dedup_fields(
    pool: &SqlitePool,
    content: &str,
    exclude_id: Option<i32>,
    quality_status: &str,
    quality_note: &str,
) -> Result<(String, String, String, Option<i32>), String> {
    let normalized = normalize_question_content(content);
    let content_hash = hash_normalized_content(&normalized);
    let duplicate_of = if normalized.is_empty() {
        None
    } else {
        find_duplicate_question_id(pool, &content_hash, exclude_id).await?
    };

    let mut final_status = quality_status.trim();
    if final_status.is_empty() {
        final_status = "unchecked";
    }

    let mut final_note = quality_note.trim().to_string();
    if let Some(id) = duplicate_of {
        final_status = "needs_review";
        let duplicate_note = format!("疑似重复：与题目 #{} 标准化题干一致，请审核是否合并。", id);
        if final_note.is_empty() {
            final_note = duplicate_note;
        } else if !final_note.contains(&duplicate_note) {
            final_note = format!("{}；{}", final_note, duplicate_note);
        }
    }

    Ok((
        content_hash,
        final_status.to_string(),
        final_note,
        duplicate_of,
    ))
}

async fn backfill_question_content_hashes(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let rows: Vec<(i32, String)> = sqlx::query_as(
        "SELECT id, content FROM questions WHERE content_hash = '' OR content_hash IS NULL",
    )
    .fetch_all(pool)
    .await?;

    for (id, content) in rows {
        let content_hash = question_content_hash(&content);
        sqlx::query("UPDATE questions SET content_hash = ? WHERE id = ?")
            .bind(content_hash)
            .bind(id)
            .execute(pool)
            .await?;
    }

    Ok(())
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
    source: &str,
    quality_status: &str,
    quality_note: &str,
) -> Result<i64, String> {
    let content = content.trim();
    if content.is_empty() {
        return Err("Question content cannot be empty".into());
    }
    if let Some(existing_id) = find_question_id_by_content(pool, content).await? {
        return Ok(existing_id as i64);
    }

    let (content_hash, final_quality_status, final_quality_note, duplicate_of) =
        prepare_question_dedup_fields(pool, content, None, quality_status, quality_note).await?;

    let result = sqlx::query(
        "INSERT INTO questions
            (question_type, content, options, tags, difficulty, standard_answer, explanation, source, quality_status, quality_note, content_hash, duplicate_of)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(question_type)
    .bind(content)
    .bind(options)
    .bind(tags)
    .bind(difficulty)
    .bind(standard_answer)
    .bind(explanation)
    .bind(source)
    .bind(final_quality_status)
    .bind(final_quality_note)
    .bind(content_hash)
    .bind(duplicate_of)
    .execute(pool)
    .await
    .map_err(|e| format!("数据库操作失败: {}", e))?;

    Ok(result.last_insert_rowid())
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
    source: &str,
    quality_status: &str,
    quality_note: &str,
) -> Result<(), String> {
    let content = content.trim();
    if content.is_empty() {
        return Err("Question content cannot be empty".into());
    }
    let (content_hash, final_quality_status, final_quality_note, duplicate_of) =
        prepare_question_dedup_fields(pool, content, Some(id), quality_status, quality_note)
            .await?;

    sqlx::query(
        "UPDATE questions
         SET question_type = ?, content = ?, options = ?, tags = ?,
             difficulty = ?, standard_answer = ?, explanation = ?, source = ?, quality_status = ?, quality_note = ?, content_hash = ?, duplicate_of = ?
         WHERE id = ?",
    )
    .bind(question_type)
    .bind(content)
    .bind(options)
    .bind(tags)
    .bind(difficulty)
    .bind(standard_answer)
    .bind(explanation)
    .bind(source)
    .bind(final_quality_status)
    .bind(final_quality_note)
    .bind(content_hash)
    .bind(duplicate_of)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| format!("数据库操作失败: {}", e))?;
    Ok(())
}

pub async fn export_questions_json(pool: &SqlitePool) -> Result<String, String> {
    let rows =
        sqlx::query_as::<_, crate::models::Question>("SELECT * FROM questions ORDER BY id ASC")
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
                            "source": q.source,
                            "quality_status": q.quality_status,
                            "quality_note": q.quality_note,
                            "content_hash": q.content_hash,
                            "duplicate_of": q.duplicate_of,
            })
        })
        .collect();

    serde_json::to_string_pretty(&items).map_err(|e| format!("序列化失败: {}", e))
}

pub async fn mark_question_wrong(pool: &SqlitePool, question_id: i32) -> Result<(), String> {
    sqlx::query("INSERT OR IGNORE INTO wrong_book_manual (question_id) VALUES (?)")
        .bind(question_id)
        .execute(pool)
        .await
        .map_err(|e| format!("加入错题本失败: {}", e))?;
    Ok(())
}

pub async fn create_resume(
    pool: &SqlitePool,
    raw_text: &str,
    candidate: &str,
    projects_json: &str,
    tech_stack_json: &str,
) -> Result<i64, String> {
    // 同 create_interview2：用 execute()+last_insert_rowid() 确保提交后行立即对异连接可见
    let result = sqlx::query(
        "INSERT INTO resumes (raw_text, candidate, projects, tech_stack)
         VALUES (?, ?, ?, ?)",
    )
    .bind(raw_text)
    .bind(candidate)
    .bind(projects_json)
    .bind(tech_stack_json)
    .execute(pool)
    .await
    .map_err(|e| format!("保存简历失败: {}", e))?;
    Ok(result.last_insert_rowid())
}

// 读取简历的 (candidate, projects_json, tech_stack_json)
pub async fn get_resume_raw(
    pool: &SqlitePool,
    id: i64,
) -> Result<(String, String, String), String> {
    sqlx::query_as::<_, (String, String, String)>(
        "SELECT candidate, projects, tech_stack FROM resumes WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("读取简历失败: {}", e))
}

pub async fn create_interview2(
    pool: &SqlitePool,
    resume_id: i64,
    project_cap: i32,
    fundamental_cap: i32,
    tags: &str,
    initial_phase: &str,
    target_role: &str,
    direction: &str,
    interview_difficulty: &str,
    follow_up_intensity: &str,
    practice_mode: &str,
) -> Result<i64, String> {
    let result = sqlx::query(
        "INSERT INTO mock_interviews (tags, question_count, status, resume_id, project_cap, fundamental_cap, phase, target_role, direction, interview_difficulty, follow_up_intensity, practice_mode)
         VALUES (?, 0, 'active', ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(tags)
    .bind(resume_id)
    .bind(project_cap)
    .bind(fundamental_cap)
    .bind(initial_phase)
    .bind(target_role)
    .bind(direction)
    .bind(interview_difficulty)
    .bind(follow_up_intensity)
    .bind(practice_mode)
    .execute(pool)
    .await
    .map_err(|e| format!("数据库操作失败: {}", e))?;
    Ok(result.last_insert_rowid())
}

pub async fn add_interview_message(
    pool: &SqlitePool,
    interview_id: i64,
    role: &str,
    phase: &str,
    content: &str,
) -> Result<(), String> {
    let next_seq: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(seq), 0) + 1 FROM interview_messages WHERE interview_id = ?",
    )
    .bind(interview_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("计算消息序号失败: {}", e))?;

    sqlx::query(
        "INSERT INTO interview_messages (interview_id, role, phase, content, seq)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(interview_id)
    .bind(role)
    .bind(phase)
    .bind(content)
    .bind(next_seq)
    .execute(pool)
    .await
    .map_err(|e| format!("保存对话消息失败: {}", e))?;
    Ok(())
}

pub async fn get_interview_messages(
    pool: &SqlitePool,
    interview_id: i64,
) -> Result<Vec<crate::models::InterviewMessage>, String> {
    sqlx::query_as::<_, (String, String, String, i64)>(
        "SELECT role, phase, content, seq FROM interview_messages
         WHERE interview_id = ? ORDER BY seq ASC",
    )
    .bind(interview_id)
    .fetch_all(pool)
    .await
    .map(|rows| {
        rows.into_iter()
            .map(
                |(role, phase, content, seq)| crate::models::InterviewMessage {
                    role,
                    phase,
                    content,
                    seq,
                },
            )
            .collect()
    })
    .map_err(|e| format!("读取对话失败: {}", e))
}

// 某环节面试官已提问的轮数
pub async fn count_phase_questions(
    pool: &SqlitePool,
    interview_id: i64,
    phase: &str,
) -> Result<i64, String> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM interview_messages
         WHERE interview_id = ? AND phase = ? AND role = 'interviewer'",
    )
    .bind(interview_id)
    .bind(phase)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("统计提问数失败: {}", e))
}

pub async fn get_interview_phase(
    pool: &SqlitePool,
    interview_id: i64,
) -> Result<
    (
        String,
        i32,
        i32,
        i64,
        String,
        String,
        String,
        String,
        String,
    ),
    String,
> {
    sqlx::query_as::<_, (String, i32, i32, i64, String, String, String, String, String)>(
        "SELECT phase, project_cap, fundamental_cap, COALESCE(resume_id, 0), target_role, direction, interview_difficulty, follow_up_intensity, practice_mode FROM mock_interviews WHERE id = ?",
    )
    .bind(interview_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("数据库操作失败: {}", e))
}

pub async fn set_interview_phase(
    pool: &SqlitePool,
    interview_id: i64,
    phase: &str,
) -> Result<(), String> {
    sqlx::query("UPDATE mock_interviews SET phase = ? WHERE id = ?")
        .bind(phase)
        .bind(interview_id)
        .execute(pool)
        .await
        .map_err(|e| format!("更新环节失败: {}", e))?;
    Ok(())
}

pub async fn finish_interview2(
    pool: &SqlitePool,
    interview_id: i64,
    average_score: f64,
    dimension_scores_json: &str,
    summary: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE mock_interviews
         SET ended_at = datetime('now','localtime'), average_score = ?, dimension_scores = ?, summary = ?, status = 'finished'
         WHERE id = ?",
    )
    .bind(average_score)
    .bind(dimension_scores_json)
    .bind(summary)
    .bind(interview_id)
    .execute(pool)
    .await
    .map_err(|e| format!("保存面试总结失败: {}", e))?;
    Ok(())
}

// 已完成面试列表：返回 (id, created_at, candidate, tags, average_score, dimension_scores_json)
pub async fn list_finished_interviews(
    pool: &SqlitePool,
) -> Result<Vec<(i64, String, String, String, f64, String, String, String)>, String> {
    sqlx::query_as::<_, (i64, String, String, String, f64, String, String, String)>(
        "SELECT mi.id, mi.created_at, COALESCE(r.candidate, ''), mi.tags, mi.average_score, mi.dimension_scores, mi.target_role, mi.direction
         FROM mock_interviews mi
         LEFT JOIN resumes r ON mi.resume_id = r.id
         WHERE mi.status = 'finished'
         ORDER BY mi.id DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Load interview history failed: {}", e))
}

pub async fn get_interview_meta(
    pool: &SqlitePool,
    interview_id: i64,
) -> Result<(f64, String, String), String> {
    sqlx::query_as::<_, (f64, String, String)>(
        "SELECT average_score, dimension_scores, summary FROM mock_interviews WHERE id = ?",
    )
    .bind(interview_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("读取面试详情失败: {}", e))
}

// 删除面试及其对话（显式两步，不依赖 FK cascade）
pub async fn delete_interview_cascade(pool: &SqlitePool, interview_id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM interview_messages WHERE interview_id = ?")
        .bind(interview_id)
        .execute(pool)
        .await
        .map_err(|e| format!("删除对话失败: {}", e))?;
    sqlx::query("DELETE FROM mock_interviews WHERE id = ?")
        .bind(interview_id)
        .execute(pool)
        .await
        .map_err(|e| format!("删除面试失败: {}", e))?;
    Ok(())
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
            explanation     TEXT    NOT NULL DEFAULT '',
            source          TEXT    NOT NULL DEFAULT '',
            quality_status  TEXT    NOT NULL DEFAULT 'unchecked',
            quality_note    TEXT    NOT NULL DEFAULT '',
            content_hash    TEXT    NOT NULL DEFAULT '',
            duplicate_of    INTEGER
        );",
    )
    .execute(&pool)
    .await?;

    let _ = sqlx::query("ALTER TABLE questions ADD COLUMN explanation TEXT NOT NULL DEFAULT ''")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE questions ADD COLUMN source TEXT NOT NULL DEFAULT ''")
        .execute(&pool)
        .await;
    let _ = sqlx::query(
        "ALTER TABLE questions ADD COLUMN quality_status TEXT NOT NULL DEFAULT 'unchecked'",
    )
    .execute(&pool)
    .await;
    let _ = sqlx::query("ALTER TABLE questions ADD COLUMN quality_note TEXT NOT NULL DEFAULT ''")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE questions ADD COLUMN content_hash TEXT NOT NULL DEFAULT ''")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE questions ADD COLUMN duplicate_of INTEGER")
        .execute(&pool)
        .await;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_questions_content_hash ON questions(content_hash)")
        .execute(&pool)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS training_sessions (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at    TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
            total_count   INTEGER NOT NULL,
            correct_count INTEGER NOT NULL,
            average_score REAL    NOT NULL,
            skipped_count INTEGER NOT NULL DEFAULT 0,
            tags          TEXT    NOT NULL DEFAULT ''
        );",
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
        );",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS topics (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT    NOT NULL UNIQUE,
            description TEXT    NOT NULL DEFAULT '',
            created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
        );",
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
            status        TEXT    NOT NULL DEFAULT 'active',
            target_role   TEXT    NOT NULL DEFAULT '',
            direction     TEXT    NOT NULL DEFAULT '',
            interview_difficulty TEXT NOT NULL DEFAULT 'standard',
            follow_up_intensity  TEXT NOT NULL DEFAULT 'normal',
            practice_mode TEXT    NOT NULL DEFAULT 'balanced'
        );",
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
        );",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS wrong_book_manual (
            question_id INTEGER PRIMARY KEY REFERENCES questions(id) ON DELETE CASCADE,
            created_at  TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        );",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS resumes (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at  TEXT NOT NULL DEFAULT (datetime('now','localtime')),
            raw_text    TEXT NOT NULL,
            candidate   TEXT NOT NULL DEFAULT '',
            projects    TEXT NOT NULL DEFAULT '[]',
            tech_stack  TEXT NOT NULL DEFAULT '[]'
        );",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS interview_messages (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            interview_id INTEGER NOT NULL REFERENCES mock_interviews(id) ON DELETE CASCADE,
            role         TEXT NOT NULL,
            phase        TEXT NOT NULL,
            content      TEXT NOT NULL,
            seq          INTEGER NOT NULL,
            created_at   TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        );",
    )
    .execute(&pool)
    .await?;

    // 扩展 mock_interviews（幂等：列已存在时忽略错误，沿用本文件既有 ALTER 风格）
    let _ = sqlx::query("ALTER TABLE mock_interviews ADD COLUMN resume_id INTEGER")
        .execute(&pool)
        .await;
    let _ = sqlx::query(
        "ALTER TABLE mock_interviews ADD COLUMN project_cap INTEGER NOT NULL DEFAULT 5",
    )
    .execute(&pool)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE mock_interviews ADD COLUMN fundamental_cap INTEGER NOT NULL DEFAULT 5",
    )
    .execute(&pool)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE mock_interviews ADD COLUMN dimension_scores TEXT NOT NULL DEFAULT '{}'",
    )
    .execute(&pool)
    .await;
    let _ =
        sqlx::query("ALTER TABLE mock_interviews ADD COLUMN phase TEXT NOT NULL DEFAULT 'project'")
            .execute(&pool)
            .await;
    let _ =
        sqlx::query("ALTER TABLE mock_interviews ADD COLUMN target_role TEXT NOT NULL DEFAULT ''")
            .execute(&pool)
            .await;
    let _ =
        sqlx::query("ALTER TABLE mock_interviews ADD COLUMN direction TEXT NOT NULL DEFAULT ''")
            .execute(&pool)
            .await;
    let _ = sqlx::query("ALTER TABLE mock_interviews ADD COLUMN interview_difficulty TEXT NOT NULL DEFAULT 'standard'").execute(&pool).await;
    let _ = sqlx::query(
        "ALTER TABLE mock_interviews ADD COLUMN follow_up_intensity TEXT NOT NULL DEFAULT 'normal'",
    )
    .execute(&pool)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE mock_interviews ADD COLUMN practice_mode TEXT NOT NULL DEFAULT 'balanced'",
    )
    .execute(&pool)
    .await;

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
    backfill_question_content_hashes(&pool).await?;
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
            &pool,
            "ESSAY",
            "什么是所有权？",
            None,
            "Rust",
            2,
            "Rust 的所有权机制……",
            "解析……",
            "手动录入",
            "reviewed",
            "",
        )
        .await
        .unwrap();
        assert!(id > 0);

        update_question(
            &pool,
            id as i32,
            "ESSAY",
            "什么是所有权与借用？",
            None,
            "Rust",
            3,
            "更新后的答案",
            "更新后的解析",
            "手动录入",
            "reviewed",
            "",
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

    #[tokio::test]
    async fn manual_wrong_mark_and_clear() {
        let db_path = test_db_path("manual-wrong");
        let pool = init_db(db_path.clone()).await.unwrap();

        let id = create_question(
            &pool,
            "ESSAY",
            "手动标记测试题",
            None,
            "其他",
            1,
            "答案",
            "解析",
            "手动录入",
            "reviewed",
            "",
        )
        .await
        .unwrap() as i32;

        mark_question_wrong(&pool, id).await.unwrap();
        let cnt: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM wrong_book_manual WHERE question_id = ?")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(cnt, 1);

        // 重复标记应幂等
        mark_question_wrong(&pool, id).await.unwrap();
        let cnt2: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM wrong_book_manual WHERE question_id = ?")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(cnt2, 1);

        pool.close().await;
        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn resume_and_interview_message_roundtrip() {
        let db_path = test_db_path("interview2");
        let pool = init_db(db_path.clone()).await.unwrap();

        let resume_id = create_resume(&pool, "raw resume text", "张三", "[]", r#"["Rust"]"#)
            .await
            .unwrap();
        assert!(resume_id > 0);

        let iv_id = create_interview2(
            &pool,
            resume_id,
            5,
            5,
            "Rust",
            "project",
            "后端开发工程师",
            "Rust",
            "standard",
            "normal",
            "balanced",
        )
        .await
        .unwrap();
        assert!(iv_id > 0);

        add_interview_message(&pool, iv_id, "interviewer", "project", "介绍下你的项目？")
            .await
            .unwrap();
        add_interview_message(&pool, iv_id, "candidate", "project", "我做了一个...")
            .await
            .unwrap();
        add_interview_message(&pool, iv_id, "interviewer", "project", "为什么这样设计？")
            .await
            .unwrap();

        let msgs = get_interview_messages(&pool, iv_id).await.unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].seq, 1);
        assert_eq!(msgs[2].seq, 3);

        // 面试官在 project 环节提了 2 个问题
        let asked = count_phase_questions(&pool, iv_id, "project")
            .await
            .unwrap();
        assert_eq!(asked, 2);

        pool.close().await;
        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn get_interview_phase_after_create_fresh() {
        let db_path = test_db_path("phase-fresh");
        let pool = init_db(db_path.clone()).await.unwrap();
        let resume_id = create_resume(&pool, "txt", "张三", "[]", r#"["Rust"]"#)
            .await
            .unwrap();
        let iv_id = create_interview2(
            &pool,
            resume_id,
            4,
            6,
            "Rust",
            "project",
            "后端开发工程师",
            "Rust",
            "standard",
            "normal",
            "balanced",
        )
        .await
        .unwrap();

        let (phase, pcap, fcap, rid, ..) = get_interview_phase(&pool, iv_id).await.unwrap();
        assert_eq!(phase, "project");
        assert_eq!(pcap, 4);
        assert_eq!(fcap, 6);
        assert_eq!(rid, resume_id);

        pool.close().await;
        let _ = std::fs::remove_file(db_path);
    }

    // 复现用户场景：已存在旧版 mock_interviews（无新列），再 init_db 迁移后读取
    #[tokio::test]
    async fn get_interview_phase_after_migrating_old_db() {
        let db_path = test_db_path("phase-migrated");
        // 1. 先用旧版 schema 建库（无 resume_id/project_cap/fundamental_cap/phase/dimension_scores）
        {
            let opts = SqliteConnectOptions::new()
                .filename(&db_path)
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal);
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(opts)
                .await
                .unwrap();
            sqlx::query(
                "CREATE TABLE mock_interviews (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
                    ended_at TEXT,
                    tags TEXT NOT NULL DEFAULT '',
                    question_count INTEGER NOT NULL,
                    average_score REAL NOT NULL DEFAULT 0,
                    summary TEXT NOT NULL DEFAULT '',
                    status TEXT NOT NULL DEFAULT 'active'
                );",
            )
            .execute(&pool)
            .await
            .unwrap();
            // 插一条旧数据，模拟历史面试
            sqlx::query("INSERT INTO mock_interviews (tags, question_count) VALUES ('Java', 3)")
                .execute(&pool)
                .await
                .unwrap();
            pool.close().await;
        }
        // 2. init_db 迁移
        let pool = init_db(db_path.clone()).await.unwrap();
        let resume_id = create_resume(&pool, "txt", "李四", "[]", r#"["Java"]"#)
            .await
            .unwrap();
        let iv_id = create_interview2(
            &pool,
            resume_id,
            5,
            5,
            "Java",
            "project",
            "Java 后端",
            "Java",
            "standard",
            "normal",
            "balanced",
        )
        .await
        .unwrap();

        // 3. 读取面试状态（用户报错处）
        let (phase, pcap, fcap, rid, ..) = get_interview_phase(&pool, iv_id).await.unwrap();
        assert_eq!(phase, "project");
        assert_eq!(pcap, 5);
        assert_eq!(fcap, 5);
        assert_eq!(rid, resume_id);

        pool.close().await;
        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn list_and_delete_interview_roundtrip() {
        let db_path = test_db_path("history");
        let pool = init_db(db_path.clone()).await.unwrap();

        let resume_id = create_resume(&pool, "txt", "王五", "[]", r#"["Go"]"#)
            .await
            .unwrap();
        let iv_id = create_interview2(
            &pool,
            resume_id,
            5,
            5,
            "Go",
            "project",
            "Go 后端",
            "Go",
            "standard",
            "normal",
            "balanced",
        )
        .await
        .unwrap();
        add_interview_message(&pool, iv_id, "interviewer", "project", "介绍项目？")
            .await
            .unwrap();
        add_interview_message(&pool, iv_id, "candidate", "project", "我做了X")
            .await
            .unwrap();
        finish_interview2(
            &pool,
            iv_id,
            80.0,
            r#"{"project_depth":85,"fundamental_solidity":75,"communication":80}"#,
            "总体不错",
        )
        .await
        .unwrap();

        let rows = list_finished_interviews(&pool).await.unwrap();
        assert_eq!(rows.len(), 1);
        let (id, _created, candidate, tags, avg, dim_json, _target_role, _direction) = &rows[0];
        assert_eq!(*id, iv_id);
        assert_eq!(candidate, "王五");
        assert_eq!(tags, "Go");
        assert!((*avg - 80.0).abs() < 1e-6);
        assert!(dim_json.contains("project_depth"));

        let (avg2, dim2, summary) = get_interview_meta(&pool, iv_id).await.unwrap();
        assert!((avg2 - 80.0).abs() < 1e-6);
        assert!(dim2.contains("85"));
        assert_eq!(summary, "总体不错");

        delete_interview_cascade(&pool, iv_id).await.unwrap();
        assert_eq!(list_finished_interviews(&pool).await.unwrap().len(), 0);
        let msg_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM interview_messages WHERE interview_id = ?")
                .bind(iv_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(msg_count, 0);

        pool.close().await;
        let _ = std::fs::remove_file(db_path);
    }
    #[test]
    fn normalize_question_content_collapses_spacing_punctuation_and_case() {
        let a = question_content_hash("TCP 三次握手的目的是什么？");
        let b = question_content_hash("tcp三次握手的目的是什么");
        let c = question_content_hash("TCP：三次握手 的 目的 是什么！！");

        assert_eq!(
            normalize_question_content("TCP 三次握手的目的是什么？"),
            "tcp三次握手的目的是什么"
        );
        assert_eq!(a, b);
        assert_eq!(b, c);
    }

    #[tokio::test]
    async fn create_question_marks_normalized_duplicate_for_review() {
        let db_path = test_db_path("question-dedup");
        let pool = init_db(db_path.clone()).await.unwrap();

        let original_id = create_question(
            &pool,
            "ESSAY",
            "TCP 三次握手的目的是什么？",
            None,
            "计算机网络",
            2,
            "建立可靠连接。",
            "三次握手用于同步序列号并确认双方收发能力。",
            "手动录入",
            "reviewed",
            "",
        )
        .await
        .unwrap();

        let duplicate_id = create_question(
            &pool,
            "ESSAY",
            "tcp三次握手的目的是什么",
            None,
            "计算机网络",
            2,
            "建立可靠连接。",
            "同一知识点的题干变体。",
            "手动录入",
            "unchecked",
            "",
        )
        .await
        .unwrap();

        let row: (String, Option<i32>, String, String) = sqlx::query_as(
            "SELECT content_hash, duplicate_of, quality_status, quality_note FROM questions WHERE id = ?",
        )
        .bind(duplicate_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert!(!row.0.is_empty());
        assert_eq!(row.1, Some(original_id as i32));
        assert_eq!(row.2, "needs_review");
        assert!(row.3.contains("疑似重复"));
        assert!(row.3.contains(&format!("#{}", original_id)));

        pool.close().await;
        let _ = std::fs::remove_file(db_path);
    }
}
