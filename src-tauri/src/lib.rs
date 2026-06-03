mod db;
mod models;
mod llm_client;
mod config;

use crate::config::AppConfig;
use crate::models::{
    DashboardStats, DayPoint, EvaluateResponse, GeneratedQuestion, GenerateProgress,
    ImportProgress, ImportQuestion, ImportResult, MockEvaluation, MockInterviewReport,
    MockInterviewStart, MockInterviewTurn, Question, SaveRecordInput, SessionRecord, Topic,
    TagStat, WrongQuestion,
    ParsedResume, ResumeRecord, InterviewTurn,
};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;
use tauri::Emitter;

// config_dir 单独作为 state，用 newtype 避免与其他 PathBuf 冲突
struct ConfigDir(PathBuf);

// ── 配置相关命令 ──────────────────────────────────────────

#[tauri::command]
fn get_api_config(
    config: tauri::State<'_, Mutex<AppConfig>>,
) -> Result<AppConfig, String> {
    config.lock().map(|c| c.clone()).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_api_config(
    api_key: String,
    api_url: String,
    model: String,
    config: tauri::State<'_, Mutex<AppConfig>>,
    config_dir: tauri::State<'_, ConfigDir>,
) -> Result<(), String> {
    let mut cfg = config.lock().map_err(|e| e.to_string())?;
    cfg.api_key = api_key;
    cfg.api_url = api_url;
    cfg.model = if model.trim().is_empty() { cfg.model.clone() } else { model };
    cfg.save(&config_dir.0)
}

// ── 训练记录命令 ──────────────────────────────────────────

#[tauri::command]
async fn save_training_session(
    records: Vec<SaveRecordInput>,
    tags: Vec<String>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    if records.is_empty() { return Ok(()); }

    let total = records.len() as i32;
    let correct = records.iter().filter(|r| {
        if r.skipped { return false; }
        r.is_correct.unwrap_or(false) || (!r.is_correct.is_some() && r.score >= 60)
    }).count() as i32;
    let skipped = records.iter().filter(|r| r.skipped).count() as i32;
    let scored: Vec<_> = records.iter().filter(|r| r.score >= 0).collect();
    let avg = if scored.is_empty() { 0.0 }
              else { scored.iter().map(|r| r.score as f64).sum::<f64>() / scored.len() as f64 };

    let session_id: i64 = sqlx::query_scalar(
        "INSERT INTO training_sessions (total_count, correct_count, average_score, skipped_count, tags)
         VALUES (?, ?, ?, ?, ?) RETURNING id"
    )
    .bind(total)
    .bind(correct)
    .bind(avg)
    .bind(skipped)
    .bind(tags.join(","))
    .fetch_one(&*pool)
    .await
    .map_err(|e| format!("保存训练会话失败: {}", e))?;

    for r in &records {
        sqlx::query(
            "INSERT INTO training_records
                (session_id, question_id, user_answer, score, is_correct, skipped, manually_added, time_spent)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(session_id)
        .bind(r.question_id)
        .bind(&r.user_answer)
        .bind(r.score)
        .bind(r.is_correct.map(|b| b as i32))
        .bind(r.skipped as i32)
        .bind(r.manually_added as i32)
        .bind(r.time_spent)
        .execute(&*pool)
        .await
        .map_err(|e| format!("保存题目记录失败: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
async fn get_wrong_questions(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<WrongQuestion>, String> {
    sqlx::query_as::<_, WrongQuestion>(
        "SELECT
            q.id          AS question_id,
            q.content,
            q.question_type,
            q.tags,
            q.difficulty,
            q.standard_answer,
            q.explanation,
            COALESCE(r.wrong_count, 0)                       AS wrong_count,
            COALESCE(r.last_score, -1)                       AS last_score,
            COALESCE(r.last_attempt, m.created_at, '')       AS last_attempt,
            COALESCE(r.manually_added_count, 0)
                + CASE WHEN m.question_id IS NOT NULL THEN 1 ELSE 0 END
                                                             AS manually_added_count
         FROM questions q
         LEFT JOIN (
            SELECT question_id,
                   COUNT(*)              AS wrong_count,
                   MAX(score)            AS last_score,
                   MAX(created_at)       AS last_attempt,
                   SUM(manually_added)   AS manually_added_count
            FROM training_records
            WHERE score < 60 OR is_correct = 0 OR manually_added = 1
            GROUP BY question_id
         ) r ON r.question_id = q.id
         LEFT JOIN wrong_book_manual m ON m.question_id = q.id
         WHERE r.question_id IS NOT NULL OR m.question_id IS NOT NULL
         ORDER BY wrong_count DESC, last_attempt DESC"
    )
    .fetch_all(&*pool)
    .await
    .map_err(|e| format!("查询错题本失败: {}", e))
}

#[tauri::command]
async fn remove_from_wrong_book(
    question_id: i32,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("DELETE FROM training_records WHERE question_id = ?")
        .bind(question_id)
        .execute(&*pool)
        .await
        .map_err(|e| format!("删除失败: {}", e))?;
    sqlx::query("DELETE FROM wrong_book_manual WHERE question_id = ?")
        .bind(question_id)
        .execute(&*pool)
        .await
        .map_err(|e| format!("删除失败: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn generate_interview_from_ids(
    question_ids: Vec<i32>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<Question>, String> {
    if question_ids.is_empty() {
        return Err("没有可练习的错题".into());
    }
    let placeholders = question_ids.iter().enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT * FROM questions WHERE id IN ({}) ORDER BY RANDOM()",
        placeholders
    );
    let mut query = sqlx::query_as::<_, Question>(&sql);
    for id in &question_ids {
        query = query.bind(id);
    }
    query.fetch_all(&*pool).await.map_err(|e| format!("组卷失败: {}", e))
}

// ── 题库命令 ──────────────────────────────────────────────

#[tauri::command]
async fn list_topics(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<Topic>, String> {
    db::list_topics(&pool)
        .await
        .map_err(|e| format!("读取考点失败: {}", e))
}

#[tauri::command]
async fn create_topic(
    name: String,
    description: Option<String>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Topic, String> {
    db::create_topic(&pool, &name, description.as_deref().unwrap_or("")).await
}

#[tauri::command]
async fn start_mock_interview(
    tags: Vec<String>,
    count: u32,
    difficulty: Option<i32>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<MockInterviewStart, String> {
    let count = count.max(1).min(20) as i64;
    let difficulty = difficulty.unwrap_or(5).clamp(1, 5);
    let mut sql = String::from("SELECT * FROM questions WHERE difficulty <= ?");
    let clean_tags: Vec<String> = tags
        .into_iter()
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty())
        .collect();

    if !clean_tags.is_empty() {
        let filters = clean_tags.iter().map(|_| "tags LIKE ?").collect::<Vec<_>>().join(" OR ");
        sql.push_str(&format!(" AND ({})", filters));
    }
    sql.push_str(" ORDER BY RANDOM() LIMIT ?");

    let mut query = sqlx::query_as::<_, Question>(&sql).bind(difficulty);
    for tag in &clean_tags {
        query = query.bind(format!("%{}%", tag));
    }
    query = query.bind(count);

    let questions = query
        .fetch_all(&*pool)
        .await
        .map_err(|e| format!("生成模拟面试题失败: {}", e))?;

    if questions.is_empty() {
        return Err("当前条件下没有可用题目，请换一个考点或难度。".into());
    }

    let interview_id: i64 = sqlx::query_scalar(
        "INSERT INTO mock_interviews (tags, question_count, status)
         VALUES (?, ?, 'active')
         RETURNING id",
    )
    .bind(clean_tags.join(","))
    .bind(questions.len() as i64)
    .fetch_one(&*pool)
    .await
    .map_err(|e| format!("创建模拟面试失败: {}", e))?;

    Ok(MockInterviewStart { interview_id, questions })
}

#[tauri::command]
async fn submit_mock_answer(
    interview_id: i64,
    question_id: i32,
    user_answer: String,
    pool: tauri::State<'_, SqlitePool>,
    config: tauri::State<'_, Mutex<AppConfig>>,
) -> Result<MockEvaluation, String> {
    if user_answer.trim().is_empty() {
        return Err("回答不能为空。".into());
    }

    let question = sqlx::query_as::<_, Question>("SELECT * FROM questions WHERE id = ?")
        .bind(question_id)
        .fetch_one(&*pool)
        .await
        .map_err(|e| format!("读取题目失败: {}", e))?;

    let (api_url, api_key, model) = {
        let cfg = config.lock().map_err(|e| e.to_string())?;
        (cfg.api_url.clone(), cfg.api_key.clone(), cfg.model.clone())
    };

    let (score, comment, follow_up) = llm_client::evaluate_mock_interview_answer(
        &api_url,
        &api_key,
        &model,
        &question.content,
        &question.standard_answer,
        &user_answer,
    )
    .await?;

    let turn_id: i64 = sqlx::query_scalar(
        "INSERT INTO mock_interview_turns
            (interview_id, question_id, question_content, user_answer, ai_comment, follow_up, score)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         RETURNING id",
    )
    .bind(interview_id)
    .bind(question_id)
    .bind(&question.content)
    .bind(user_answer.trim())
    .bind(&comment)
    .bind(&follow_up)
    .bind(score)
    .fetch_one(&*pool)
    .await
    .map_err(|e| format!("保存模拟面试回答失败: {}", e))?;

    Ok(MockEvaluation { turn_id, score, comment, follow_up })
}

#[tauri::command]
async fn submit_mock_follow_up(
    turn_id: i64,
    follow_up_answer: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("UPDATE mock_interview_turns SET follow_up_answer = ? WHERE id = ?")
        .bind(follow_up_answer.trim())
        .bind(turn_id)
        .execute(&*pool)
        .await
        .map_err(|e| format!("保存追问回答失败: {}", e))?;
    Ok(())
}

// 跳过的题目也记一条 turn（0 分、未作答），否则面试报告会漏算、误判表现
const SKIPPED_MARK: &str = "（跳过未作答）";

#[tauri::command]
async fn record_skipped_question(
    interview_id: i64,
    question_id: i32,
    question_content: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO mock_interview_turns
            (interview_id, question_id, question_content, user_answer, ai_comment, follow_up, follow_up_answer, score)
         VALUES (?, ?, ?, ?, '', '', '', 0)",
    )
    .bind(interview_id)
    .bind(question_id)
    .bind(&question_content)
    .bind(SKIPPED_MARK)
    .execute(&*pool)
    .await
    .map_err(|e| format!("记录跳过题目失败: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn finish_mock_interview(
    interview_id: i64,
    pool: tauri::State<'_, SqlitePool>,
    config: tauri::State<'_, Mutex<AppConfig>>,
) -> Result<MockInterviewReport, String> {
    let turns = sqlx::query_as::<_, MockInterviewTurn>(
        "SELECT id, interview_id, question_id, question_content, user_answer, ai_comment,
                follow_up, follow_up_answer, score, created_at
         FROM mock_interview_turns
         WHERE interview_id = ?
         ORDER BY id ASC",
    )
    .bind(interview_id)
    .fetch_all(&*pool)
    .await
    .map_err(|e| format!("读取模拟面试记录失败: {}", e))?;

    let average_score = if turns.is_empty() {
        0.0
    } else {
        turns.iter().map(|turn| turn.score as f64).sum::<f64>() / turns.len() as f64
    };

    // 真正作答过的题（排除跳过 / 空回答），用于判断这场面试是否有可评估内容
    let answered_count = turns
        .iter()
        .filter(|turn| turn.user_answer.trim() != SKIPPED_MARK && !turn.user_answer.trim().is_empty())
        .count();

    let summary = if answered_count == 0 {
        // 全部跳过 / 没有任何作答：诚实反馈，不调用 AI（否则会凭空夸奖）
        format!(
            "本次面试共 {} 道题，但全部跳过、没有任何作答，无法评估表现。建议正式作答后再生成复盘。",
            turns.len()
        )
    } else {
        let transcript = turns
            .iter()
            .map(|turn| {
                format!(
                    "题目：{}\n回答：{}\n点评：{}\n追问：{}\n追问回答：{}\n得分：{}",
                    turn.question_content,
                    turn.user_answer,
                    turn.ai_comment,
                    turn.follow_up,
                    turn.follow_up_answer,
                    turn.score
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let (api_url, api_key, model) = {
            let cfg = config.lock().map_err(|e| e.to_string())?;
            (cfg.api_url.clone(), cfg.api_key.clone(), cfg.model.clone())
        };
        llm_client::summarize_mock_interview(&api_url, &api_key, &model, &transcript)
            .await
            .unwrap_or_else(|_| {
                format!(
                    "本次模拟面试共完成 {} 题（其中作答 {} 题），平均分 {:.1}。建议复盘低分题并补充关键概念。",
                    turns.len(),
                    answered_count,
                    average_score
                )
            })
    };

    sqlx::query(
        "UPDATE mock_interviews
         SET ended_at = datetime('now', 'localtime'), average_score = ?, summary = ?, status = 'finished'
         WHERE id = ?",
    )
    .bind(average_score)
    .bind(&summary)
    .bind(interview_id)
    .execute(&*pool)
    .await
    .map_err(|e| format!("保存模拟面试总结失败: {}", e))?;

    Ok(MockInterviewReport { interview_id, average_score, summary, turns })
}

// ── 对话式模拟面试命令（简历解析 / 流式对话 / 多维评分）──────────

#[tauri::command]
async fn parse_resume(
    raw_text: String,
    pool: tauri::State<'_, SqlitePool>,
    config: tauri::State<'_, Mutex<AppConfig>>,
) -> Result<ResumeRecord, String> {
    if raw_text.trim().is_empty() {
        return Err("简历内容为空，请确认 PDF 已正确解析。".into());
    }
    let (api_url, api_key, model) = {
        let cfg = config.lock().map_err(|e| e.to_string())?;
        (cfg.api_url.clone(), cfg.api_key.clone(), cfg.model.clone())
    };

    let parsed: ParsedResume =
        llm_client::parse_resume_llm(&api_url, &api_key, &model, &raw_text).await?;

    let projects_json = serde_json::to_string(&parsed.projects).unwrap_or_else(|_| "[]".into());
    let tech_stack_json = serde_json::to_string(&parsed.tech_stack).unwrap_or_else(|_| "[]".into());

    let id = db::create_resume(&pool, &raw_text, &parsed.candidate, &projects_json, &tech_stack_json).await?;

    Ok(ResumeRecord {
        id,
        candidate: parsed.candidate,
        projects: parsed.projects,
        tech_stack: parsed.tech_stack,
    })
}

// 构建某环节的 system prompt
fn build_interviewer_system(phase: &str, resume_brief: &str, used: i32, cap: i32) -> String {
    let remaining = (cap - used).max(0);
    let phase_cn = if phase == "project" { "项目经历" } else { "技术八股（基础原理）" };
    format!(
        "你是一名资深技术面试官，正在进行模拟面试的【{phase_cn}】环节。\
        候选人简历摘要：{resume_brief}。\
        要求：每次只问一个问题；问题要顺着候选人上一轮回答自然深入或转向；语气专业简洁，不要寒暄过多；不要给出答案或点评。\
        本环节还可进行约 {remaining} 轮。若你认为本环节已充分考察，可在回复最后另起一行输出 {mark} 表示提前结束本环节。\
        {phase_extra}",
        phase_cn = phase_cn,
        resume_brief = resume_brief,
        remaining = remaining,
        mark = PHASE_DONE_MARK,
        phase_extra = if phase == "project" {
            "项目环节：围绕候选人真实项目追问技术选型、难点、权衡、量化结果。"
        } else {
            "八股环节：围绕候选人技术栈考察底层原理、常见考点，由浅入深。"
        }
    )
}

// 取简历摘要文本（候选人 + 技术栈 + 项目名），用于注入提示词
async fn resume_brief(pool: &SqlitePool, resume_id: i64) -> String {
    match db::get_resume_raw(pool, resume_id).await {
        Ok((candidate, projects_json, tech_json)) => {
            let projects: Vec<crate::models::ResumeProject> =
                serde_json::from_str(&projects_json).unwrap_or_default();
            let tech: Vec<String> = serde_json::from_str(&tech_json).unwrap_or_default();
            let proj_names: Vec<String> = projects.iter().map(|p| p.name.clone()).collect();
            format!("候选人 {}；技术栈 [{}]；项目 [{}]", candidate, tech.join("、"), proj_names.join("、"))
        }
        Err(_) => String::new(),
    }
}

// 生成一轮面试官提问：组装消息→流式生成（发 interview-token 事件）→剥离 PHASE_DONE→落库→推进状态机
async fn run_interviewer_turn(
    pool: &SqlitePool,
    app: &tauri::AppHandle,
    api_url: &str,
    api_key: &str,
    model: &str,
    interview_id: i64,
) -> Result<InterviewTurn, String> {
    // 注：`app.emit` 依赖文件顶部已有的 `use tauri::Emitter;`（lib.rs:17），此处不再重复 import。
    let (current_phase, project_cap, fundamental_cap, resume_id) =
        db::get_interview_phase(pool, interview_id).await?;
    let used = db::count_phase_questions(pool, interview_id, &current_phase).await? as i32;

    let brief = resume_brief(pool, resume_id).await;
    let cap = if current_phase == "project" { project_cap } else { fundamental_cap };
    let system = build_interviewer_system(&current_phase, &brief, used, cap);

    // 组装消息：system + 历史（interviewer→assistant, candidate→user）
    let history = db::get_interview_messages(pool, interview_id).await?;
    let mut messages: Vec<serde_json::Value> = vec![serde_json::json!({"role":"system","content":system})];
    for m in &history {
        let role = if m.role == "interviewer" { "assistant" } else { "user" };
        messages.push(serde_json::json!({"role": role, "content": m.content}));
    }
    // 若是本环节第一问且无历史，给个起始 user 提示，促使模型开口
    if history.is_empty() {
        messages.push(serde_json::json!({"role":"user","content":"请开始面试，先做简短开场再提出第一个问题。"}));
    }

    let app_for_cb = app.clone();
    let full = llm_client::call_api_stream(
        api_url, api_key, model, messages, 0.6, 1024,
        move |token| {
            let _ = app_for_cb.emit("interview-token", serde_json::json!({
                "interviewId": interview_id,
                "chunk": token,
            }));
        },
    )
    .await?;

    let (clean_text, phase_done) = strip_phase_done(&full);
    let message = if clean_text.is_empty() { "（面试官沉默了，请稍后重试）".to_string() } else { clean_text };

    // 落库本轮面试官提问
    db::add_interview_message(pool, interview_id, "interviewer", &current_phase, &message).await?;

    // 推进状态机：基于「问完这轮后」的计数判断下一环节/是否结束
    let used_after = used + 1;
    let (project_used, fundamental_used) = if current_phase == "project" {
        (used_after, db::count_phase_questions(pool, interview_id, "fundamental").await? as i32)
    } else {
        (db::count_phase_questions(pool, interview_id, "project").await? as i32, used_after)
    };
    let (next_phase, finished) = decide_phase(
        &current_phase, project_used, project_cap, fundamental_used, fundamental_cap, phase_done,
    );
    if next_phase != current_phase {
        db::set_interview_phase(pool, interview_id, &next_phase).await?;
    }

    Ok(InterviewTurn { message, phase: current_phase, finished })
}

#[tauri::command]
async fn start_interview(
    resume_id: i64,
    project_cap: i32,
    fundamental_cap: i32,
    pool: tauri::State<'_, SqlitePool>,
    config: tauri::State<'_, Mutex<AppConfig>>,
    app: tauri::AppHandle,
) -> Result<(i64, InterviewTurn), String> {
    let (api_url, api_key, model) = {
        let cfg = config.lock().map_err(|e| e.to_string())?;
        (cfg.api_url.clone(), cfg.api_key.clone(), cfg.model.clone())
    };
    let pc = project_cap.clamp(1, 20);
    let fc = fundamental_cap.clamp(0, 20);
    let interview_id = db::create_interview2(&pool, resume_id, pc, fc, "").await?;
    let turn = run_interviewer_turn(&pool, &app, &api_url, &api_key, &model, interview_id).await?;
    Ok((interview_id, turn))
}

#[tauri::command]
async fn interview_respond(
    interview_id: i64,
    answer: String,
    pool: tauri::State<'_, SqlitePool>,
    config: tauri::State<'_, Mutex<AppConfig>>,
    app: tauri::AppHandle,
) -> Result<InterviewTurn, String> {
    let (api_url, api_key, model) = {
        let cfg = config.lock().map_err(|e| e.to_string())?;
        (cfg.api_url.clone(), cfg.api_key.clone(), cfg.model.clone())
    };
    // 落库候选人回答（用当前环节）
    let (current_phase, _pc, _fc, _rid) = db::get_interview_phase(&pool, interview_id).await?;
    let ans = if answer.trim().is_empty() { "（跳过未作答）".to_string() } else { answer.trim().to_string() };
    db::add_interview_message(&pool, interview_id, "candidate", &current_phase, &ans).await?;

    run_interviewer_turn(&pool, &app, &api_url, &api_key, &model, interview_id).await
}

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

#[tauri::command]
async fn generate_interview(
    tags: Vec<String>,
    count: u32,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<Question>, String> {
    let per_tag = count.max(1) as i64;
    let mut seen_ids = std::collections::HashSet::new();
    let mut result: Vec<Question> = Vec::new();

    for tag in tags {
        let query_tag = format!("%{}%", tag);
        let questions = sqlx::query_as::<_, Question>(
            "SELECT * FROM questions WHERE tags LIKE ? ORDER BY RANDOM() LIMIT ?",
        )
        .bind(query_tag)
        .bind(per_tag)
        .fetch_all(&*pool)
        .await
        .map_err(|e| format!("数据库查询失败: {}", e))?;

        for q in questions {
            if seen_ids.insert(q.id) {
                result.push(q);
            }
        }
    }

    if result.is_empty() {
        return Err("选中的考点下暂时没有题目，请重新选择或导入题库。".into());
    }
    Ok(result)
}

#[tauri::command]
async fn evaluate_answer(
    question_id: i32,
    user_answer: String,
    pool: tauri::State<'_, SqlitePool>,
    config: tauri::State<'_, Mutex<AppConfig>>,
) -> Result<EvaluateResponse, String> {
    let q = sqlx::query_as::<_, Question>(
        "SELECT * FROM questions WHERE id = ?",
    )
    .bind(question_id)
    .fetch_one(&*pool)
    .await
    .map_err(|e| format!("查询题目失败: {}", e))?;

    match q.question_type.as_str() {
        "SINGLE" => {
            let is_correct = user_answer.trim().eq_ignore_ascii_case(q.standard_answer.trim());
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

        "MULTI" => {
            let normalize = |s: &str| -> Vec<char> {
                let mut v: Vec<char> = s
                    .split(|c: char| !c.is_ascii_alphabetic())
                    .flat_map(|seg| seg.chars())
                    .filter(|c| c.is_ascii_uppercase())
                    .collect();
                v.sort();
                v.dedup();
                v
            };
            let user_set = normalize(user_answer.trim());
            let std_set = normalize(q.standard_answer.trim());
            let is_correct = user_set == std_set;

            let ai_comment = if is_correct {
                "✅ 回答正确！".to_string()
            } else {
                let user_str: String = user_set.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", ");
                let std_str: String = std_set.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", ");
                format!("❌ 回答有误。你选择了【{}】，正确答案是【{}】。", user_str, std_str)
            };

            Ok(EvaluateResponse {
                standard_answer: q.standard_answer,
                explanation: q.explanation,
                is_correct: Some(is_correct),
                ai_comment,
                score: if is_correct { 100 } else { 0 },
            })
        }

        "ESSAY" | _ => {
            let (api_url, api_key, model) = {
                let cfg = config.lock().map_err(|e| e.to_string())?;
                (cfg.api_url.clone(), cfg.api_key.clone(), cfg.model.clone())
            };

            let (score, ai_comment) = llm_client::evaluate_essay_answer(
                &api_url,
                &api_key,
                &model,
                &q.content,
                &q.standard_answer,
                &user_answer,
            )
            .await?;

            Ok(EvaluateResponse {
                standard_answer: q.standard_answer,
                explanation: q.explanation,
                is_correct: None,
                ai_comment,
                score,
            })
        }
    }
}

#[tauri::command]
async fn import_questions_from_file(
    file_path: String,
    pool: tauri::State<'_, SqlitePool>,
    config: tauri::State<'_, Mutex<AppConfig>>,
    app: tauri::AppHandle,
) -> Result<ImportResult, String> {
    let content = tokio::fs::read_to_string(&file_path)
        .await
        .map_err(|e| format!("文件读取失败: {}", e))?;
    let import_list: Vec<ImportQuestion> = serde_json::from_str(&content)
        .map_err(|e| format!("JSON 格式不正确: {}", e))?;
    let total = import_list.len();
    if total == 0 {
        return Err("文件内无题目".into());
    }

    let (api_url, api_key, model) = {
        let cfg = config.lock().map_err(|e| e.to_string())?;
        (cfg.api_url.clone(), cfg.api_key.clone(), cfg.model.clone())
    };
    let topic_candidates: Vec<String> = db::list_topics(&pool)
        .await
        .map_err(|e| format!("读取考点失败: {}", e))?
        .into_iter()
        .map(|topic| topic.name)
        .collect();

    let pool_clone = (*pool).clone();
    tokio::spawn(async move {
        let mut ai_count = 0;

        for (i, item) in import_list.into_iter().enumerate() {
            let current_idx = i + 1;

            let _ = app.emit("import-status", ImportProgress {
                current: current_idx,
                total,
                message: format!("正在处理: {:.30}...", item.content),
                is_finished: false,
            });

            let needs_ai = item.standard_answer.as_deref().unwrap_or("").trim().is_empty()
                || item.explanation.as_deref().unwrap_or("").trim().is_empty()
                || item.tags.trim().is_empty();

            let (ans, exp, tag) = if needs_ai {
                let options_text = item.options.as_ref().map(|o| o.join(", "));
                match llm_client::generate_answer_and_explanation_with_tags(
                    &api_url,
                    &api_key,
                    &model,
                    &topic_candidates,
                    &item.question_type,
                    &item.content,
                    options_text.as_deref(),
                )
                .await
                {
                    Ok((a, e, t)) => {
                        ai_count += 1;
                        let final_tag = if item.tags.trim().is_empty() { t } else { item.tags.clone() };
                        (a, e, final_tag)
                    }
                    Err(e) => {
                        eprintln!("⚠️ 第 {} 题 AI 补全失败: {}", current_idx, e);
                        (
                            item.standard_answer.unwrap_or_default(),
                            item.explanation.unwrap_or_default(),
                            item.tags.clone(),
                        )
                    }
                }
            } else {
                (
                    item.standard_answer.unwrap_or_default(),
                    item.explanation.unwrap_or_default(),
                    item.tags.clone(),
                )
            };

            let options_json = item.options.map(|o| serde_json::to_string(&o).unwrap_or_default());

            let res = sqlx::query(
                "INSERT INTO questions
                    (question_type, content, options, tags, difficulty, standard_answer, explanation)
                 VALUES (?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(content) DO UPDATE SET
                    standard_answer = excluded.standard_answer,
                    explanation = excluded.explanation,
                    tags = excluded.tags",
            )
            .bind(&item.question_type)
            .bind(&item.content)
            .bind(&options_json)
            .bind(&tag)
            .bind(item.difficulty)
            .bind(&ans)
            .bind(&exp)
            .execute(&pool_clone)
            .await;

            if let Err(e) = res {
                eprintln!("❌ 第 {} 题入库失败: {}", current_idx, e);
            }
        }

        let _ = app.emit("import-status", ImportProgress {
            current: total,
            total,
            message: format!("🎉 导入完成！AI 补全/规范化分类了 {} 道题目。", ai_count),
            is_finished: true,
        });
    });

    Ok(ImportResult {
        total,
        ai_generated: 0,
        message: format!("已启动后台导入，共 {} 题，正在进行 AI 语义分类...", total),
    })
}

// ── AI 出题命令 ────────────────────────────────────────────

#[tauri::command]
async fn generate_questions_by_ai(
    topic: String,
    question_type: String,
    difficulty: i32,
    count: u32,
    requirement: Option<String>,
    config: tauri::State<'_, Mutex<AppConfig>>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let (api_url, api_key, model) = {
        let cfg = config.lock().map_err(|e| e.to_string())?;
        (cfg.api_url.clone(), cfg.api_key.clone(), cfg.model.clone())
    };
    let total = count as usize;

    tokio::spawn(async move {
        for i in 0..total {
            let _ = app.emit("ai-generate-progress", GenerateProgress {
                current: i,
                total,
                question: None,
                message: format!("正在生成第 {}/{} 题...", i + 1, total),
                is_finished: false,
                error: None,
            });

            match llm_client::generate_single_question(
                &api_url, &api_key, &model, &topic, &question_type, difficulty,
                requirement.as_deref(),
            ).await {
                Ok(q) => {
                    let _ = app.emit("ai-generate-progress", GenerateProgress {
                        current: i + 1,
                        total,
                        question: Some(q),
                        message: format!("已生成 {}/{} 题", i + 1, total),
                        is_finished: false,
                        error: None,
                    });
                }
                Err(e) => {
                    let _ = app.emit("ai-generate-progress", GenerateProgress {
                        current: i + 1,
                        total,
                        question: None,
                        message: format!("第 {} 题生成失败", i + 1),
                        is_finished: false,
                        error: Some(e),
                    });
                }
            }
        }

        let _ = app.emit("ai-generate-progress", GenerateProgress {
            current: total,
            total,
            question: None,
            message: "🎉 生成完成！".to_string(),
            is_finished: true,
            error: None,
        });
    });

    Ok(())
}

#[tauri::command]
async fn save_ai_generated_questions(
    questions: Vec<GeneratedQuestion>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<usize, String> {
    let mut saved = 0usize;
    for q in &questions {
        let options_json = q.options.as_ref()
            .map(|o| serde_json::to_string(o).unwrap_or_default());

        let res = sqlx::query(
            "INSERT INTO questions
                (question_type, content, options, tags, difficulty, standard_answer, explanation)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(content) DO UPDATE SET
                standard_answer = excluded.standard_answer,
                explanation = excluded.explanation,
                tags = excluded.tags",
        )
        .bind(&q.question_type)
        .bind(&q.content)
        .bind(&options_json)
        .bind(&q.tags)
        .bind(q.difficulty)
        .bind(&q.standard_answer)
        .bind(&q.explanation)
        .execute(&*pool)
        .await;

        match res {
            Ok(_) => saved += 1,
            Err(e) => eprintln!("❌ AI 出题入库失败: {}", e),
        }
    }
    Ok(saved)
}

fn norm_tag(t: &Option<String>) -> Option<String> {
    t.as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && *s != "全部")
        .map(|s| s.to_string())
}
fn norm_search(s: &Option<String>) -> Option<String> {
    s.as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

#[tauri::command]
async fn list_questions(
    tag: Option<String>,
    search: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<Question>, String> {
    let limit = limit.unwrap_or(50).max(1).min(500);
    let offset = offset.unwrap_or(0).max(0);
    let tag = norm_tag(&tag);
    let search = norm_search(&search);

    let mut sql = String::from("SELECT * FROM questions WHERE 1=1");
    if tag.is_some() { sql.push_str(" AND tags LIKE ?"); }
    if search.is_some() {
        sql.push_str(" AND (content LIKE ? OR tags LIKE ? OR standard_answer LIKE ?)");
    }
    sql.push_str(" ORDER BY id DESC LIMIT ? OFFSET ?");

    let mut q = sqlx::query_as::<_, Question>(&sql);
    if let Some(t) = &tag { q = q.bind(format!("%{}%", t)); }
    if let Some(s) = &search {
        let like = format!("%{}%", s);
        q = q.bind(like.clone()).bind(like.clone()).bind(like);
    }
    q = q.bind(limit).bind(offset);
    q.fetch_all(&*pool).await.map_err(|e| format!("查询题库失败: {}", e))
}

#[tauri::command]
async fn delete_question(
    id: i32,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("DELETE FROM questions WHERE id = ?")
        .bind(id)
        .execute(&*pool).await
        .map_err(|e| format!("删除题目失败: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn count_questions(
    tag: Option<String>,
    search: Option<String>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    let tag = norm_tag(&tag);
    let search = norm_search(&search);

    let mut sql = String::from("SELECT COUNT(*) FROM questions WHERE 1=1");
    if tag.is_some() { sql.push_str(" AND tags LIKE ?"); }
    if search.is_some() {
        sql.push_str(" AND (content LIKE ? OR tags LIKE ? OR standard_answer LIKE ?)");
    }

    let mut q = sqlx::query_scalar::<_, i64>(&sql);
    if let Some(t) = &tag { q = q.bind(format!("%{}%", t)); }
    if let Some(s) = &search {
        let like = format!("%{}%", s);
        q = q.bind(like.clone()).bind(like.clone()).bind(like);
    }
    q.fetch_one(&*pool).await.map_err(|e| format!("统计题目数量失败: {}", e))
}

#[tauri::command]
async fn create_question(
    question_type: String,
    content: String,
    options: Option<String>,
    tags: String,
    difficulty: i32,
    standard_answer: String,
    explanation: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    db::create_question(
        &pool, &question_type, &content, options.as_deref(),
        &tags, difficulty, &standard_answer, &explanation,
    )
    .await
}

#[tauri::command]
async fn update_question(
    id: i32,
    question_type: String,
    content: String,
    options: Option<String>,
    tags: String,
    difficulty: i32,
    standard_answer: String,
    explanation: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    db::update_question(
        &pool, id, &question_type, &content, options.as_deref(),
        &tags, difficulty, &standard_answer, &explanation,
    )
    .await
}

#[tauri::command]
async fn export_questions(
    path: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<usize, String> {
    let json = db::export_questions_json(&pool).await?;
    std::fs::write(&path, &json).map_err(|e| format!("写入文件失败: {}", e))?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM questions")
        .fetch_one(&*pool)
        .await
        .map_err(|e| format!("统计失败: {}", e))?;
    Ok(count as usize)
}

#[tauri::command]
async fn mark_question_wrong(
    question_id: i32,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    db::mark_question_wrong(&pool, question_id).await
}

#[tauri::command]
async fn get_all_tags(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<String>, String> {
    let topics = db::list_topics(&pool)
        .await
        .map_err(|e| format!("读取考点失败: {}", e))?;
    let rows: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT tags FROM questions")
        .fetch_all(&*pool)
        .await
        .map_err(|e| format!("查询标签失败: {}", e))?;

    let mut tag_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    for topic in topics {
        tag_set.insert(topic.name);
    }
    for (tags_str,) in rows {
        for tag in tags_str.split(',') {
            let t = tag.trim().to_string();
            if !t.is_empty() {
                tag_set.insert(t);
            }
        }
    }

    let mut tags: Vec<String> = tag_set.into_iter().collect();
    tags.sort();
    Ok(tags)
}

#[tauri::command]
async fn get_tag_counts(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<std::collections::HashMap<String, usize>, String> {
    let topics = db::list_topics(&pool)
        .await
        .map_err(|e| format!("读取考点失败: {}", e))?;
    let rows: Vec<(String,)> = sqlx::query_as("SELECT tags FROM questions")
        .fetch_all(&*pool)
        .await
        .map_err(|e| format!("查询标签失败: {}", e))?;

    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for topic in topics {
        counts.entry(topic.name).or_insert(0);
    }
    for (tags_str,) in rows {
        for tag in tags_str.split(',') {
            let t = tag.trim().to_string();
            if !t.is_empty() {
                *counts.entry(t).or_insert(0) += 1;
            }
        }
    }
    Ok(counts)
}

// ── Dashboard 统计命令 ────────────────────────────────────

fn is_record_correct(score: i32, is_correct: Option<i32>, skipped: i32) -> Option<bool> {
    if skipped != 0 { return Some(false); }
    if let Some(c) = is_correct {
        return Some(c != 0);
    }
    Some(score >= 60)
}

#[tauri::command]
async fn get_dashboard_stats(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<DashboardStats, String> {
    // 全部记录（按日期 + 正确性聚合）
    let rows: Vec<(i32, Option<i32>, i32, String)> = sqlx::query_as(
        "SELECT score, is_correct, skipped, substr(created_at, 1, 10) AS day
         FROM training_records"
    )
    .fetch_all(&*pool)
    .await
    .map_err(|e| format!("查询训练记录失败: {}", e))?;

    let total_answered = rows.len() as i64;

    // 整体正确率（不包含 skipped）
    let mut correct_n = 0i64;
    let mut answered_n = 0i64;
    for (score, ic, sk, _) in &rows {
        if *sk != 0 { continue; }
        answered_n += 1;
        if is_record_correct(*score, *ic, *sk).unwrap_or(false) {
            correct_n += 1;
        }
    }
    let overall_accuracy = if answered_n > 0 {
        correct_n as f64 / answered_n as f64
    } else { 0.0 };

    // 今日做题数
    let today: String = sqlx::query_scalar("SELECT date('now', 'localtime')")
        .fetch_one(&*pool).await
        .map_err(|e| format!("查询日期失败: {}", e))?;
    let today_answered = rows.iter().filter(|(_, _, _, d)| *d == today).count() as i64;

    // 连续打卡：从今天往前数，每天必须有 >=1 条记录
    use std::collections::HashSet;
    let active_days: HashSet<&str> = rows.iter().map(|(_, _, _, d)| d.as_str()).collect();
    let mut streak_days = 0i64;
    // 用 SQLite 计算日期偏移
    let mut offset = 0i64;
    loop {
        let probe: String = sqlx::query_scalar(
            "SELECT date('now', 'localtime', ? || ' day')"
        )
        .bind(format!("-{}", offset))
        .fetch_one(&*pool).await
        .map_err(|e| format!("查询日期偏移失败: {}", e))?;
        if active_days.contains(probe.as_str()) {
            streak_days += 1;
            offset += 1;
        } else {
            // 今天还没做题不打断连击（仍可保留昨天起的连击）
            if offset == 0 { offset += 1; continue; }
            break;
        }
        if offset > 365 { break; }
    }

    // 本周 / 上周对比（按自然 7 天滚动）
    let week_ago: String = sqlx::query_scalar(
        "SELECT date('now', 'localtime', '-7 day')"
    ).fetch_one(&*pool).await.map_err(|e| e.to_string())?;
    let two_weeks_ago: String = sqlx::query_scalar(
        "SELECT date('now', 'localtime', '-14 day')"
    ).fetch_one(&*pool).await.map_err(|e| e.to_string())?;

    let mut this_week_total = 0i64;
    let mut this_week_correct = 0i64;
    let mut this_week_answered = 0i64;
    let mut last_week_total = 0i64;
    let mut last_week_correct = 0i64;
    let mut last_week_answered = 0i64;
    for (score, ic, sk, d) in &rows {
        let in_this = d.as_str() > week_ago.as_str();
        let in_last = d.as_str() > two_weeks_ago.as_str() && d.as_str() <= week_ago.as_str();
        if in_this {
            this_week_total += 1;
            if *sk == 0 {
                this_week_answered += 1;
                if is_record_correct(*score, *ic, *sk).unwrap_or(false) {
                    this_week_correct += 1;
                }
            }
        } else if in_last {
            last_week_total += 1;
            if *sk == 0 {
                last_week_answered += 1;
                if is_record_correct(*score, *ic, *sk).unwrap_or(false) {
                    last_week_correct += 1;
                }
            }
        }
    }
    let week_delta_answered = this_week_total - last_week_total;
    let this_acc = if this_week_answered > 0 { this_week_correct as f64 / this_week_answered as f64 } else { 0.0 };
    let last_acc = if last_week_answered > 0 { last_week_correct as f64 / last_week_answered as f64 } else { 0.0 };
    let week_delta_accuracy = (this_acc - last_acc) * 100.0;

    // 总标签数（去重）
    let tag_rows: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT tags FROM questions")
        .fetch_all(&*pool).await.map_err(|e| e.to_string())?;
    let mut all_tags: HashSet<String> = HashSet::new();
    for (s,) in tag_rows {
        for t in s.split(',') {
            let t = t.trim();
            if !t.is_empty() { all_tags.insert(t.to_string()); }
        }
    }
    let total_tags = all_tags.len() as i64;

    // 已掌握标签：复用 get_tag_mastery 逻辑
    let mastery = compute_tag_mastery(&pool).await?;
    let mastered_tags = mastery.iter().filter(|t| t.total >= 5 && t.accuracy >= 0.8).count() as i64;

    // 待复习 = 错题本条数
    let pending_review: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT q.id)
         FROM training_records r JOIN questions q ON q.id = r.question_id
         WHERE r.score < 60 OR r.is_correct = 0 OR r.manually_added = 1"
    ).fetch_one(&*pool).await.unwrap_or(0);

    Ok(DashboardStats {
        total_answered,
        overall_accuracy,
        mastered_tags,
        total_tags,
        pending_review,
        streak_days,
        today_answered,
        week_delta_answered,
        week_delta_accuracy,
    })
}

#[tauri::command]
async fn get_accuracy_trend(
    days: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<DayPoint>, String> {
    let days = days.max(1).min(180);
    let rows: Vec<(String, i32, Option<i32>, i32)> = sqlx::query_as(
        "SELECT substr(created_at, 1, 10) AS day, score, is_correct, skipped
         FROM training_records
         WHERE date(created_at) >= date('now', 'localtime', ? || ' day')
         ORDER BY day ASC"
    )
    .bind(format!("-{}", days - 1))
    .fetch_all(&*pool).await
    .map_err(|e| format!("查询趋势失败: {}", e))?;

    use std::collections::BTreeMap;
    let mut by_day: BTreeMap<String, (i64, i64)> = BTreeMap::new(); // (correct, answered)
    for (day, score, ic, sk) in rows {
        if sk != 0 { continue; }
        let entry = by_day.entry(day).or_insert((0, 0));
        entry.1 += 1;
        if is_record_correct(score, ic, sk).unwrap_or(false) {
            entry.0 += 1;
        }
    }
    Ok(by_day.into_iter().map(|(date, (c, a))| DayPoint {
        date,
        accuracy: if a > 0 { c as f64 / a as f64 } else { 0.0 },
        count: a,
    }).collect())
}

async fn compute_tag_mastery(pool: &SqlitePool) -> Result<Vec<TagStat>, String> {
    let rows: Vec<(String, i32, Option<i32>, i32)> = sqlx::query_as(
        "SELECT q.tags, r.score, r.is_correct, r.skipped
         FROM training_records r JOIN questions q ON q.id = r.question_id"
    ).fetch_all(pool).await.map_err(|e| e.to_string())?;

    use std::collections::HashMap;
    let mut by_tag: HashMap<String, (i64, i64)> = HashMap::new(); // (correct, total_answered)
    for (tags, score, ic, sk) in rows {
        if sk != 0 { continue; }
        let correct = is_record_correct(score, ic, sk).unwrap_or(false);
        for t in tags.split(',') {
            let t = t.trim();
            if t.is_empty() { continue; }
            let e = by_tag.entry(t.to_string()).or_insert((0, 0));
            e.1 += 1;
            if correct { e.0 += 1; }
        }
    }
    let mut out: Vec<TagStat> = by_tag.into_iter()
        .map(|(tag, (c, t))| TagStat {
            tag,
            accuracy: if t > 0 { c as f64 / t as f64 } else { 0.0 },
            total: t,
        }).collect();
    out.sort_by(|a, b| b.accuracy.partial_cmp(&a.accuracy).unwrap_or(std::cmp::Ordering::Equal));
    Ok(out)
}

#[tauri::command]
async fn get_tag_mastery(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<TagStat>, String> {
    compute_tag_mastery(&pool).await
}

#[tauri::command]
async fn delete_session(
    id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    // 先删 records（FK ON DELETE CASCADE 在未开启 PRAGMA 时不会自动级联）
    sqlx::query("DELETE FROM training_records WHERE session_id = ?")
        .bind(id)
        .execute(&*pool).await
        .map_err(|e| format!("删除训练记录失败: {}", e))?;
    sqlx::query("DELETE FROM training_sessions WHERE id = ?")
        .bind(id)
        .execute(&*pool).await
        .map_err(|e| format!("删除会话失败: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn get_recent_sessions(
    limit: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<SessionRecord>, String> {
    let limit = limit.max(1).min(50);
    let rows: Vec<(i64, String, i32, i32, String)> = sqlx::query_as(
        "SELECT id, created_at, total_count, correct_count, tags
         FROM training_sessions
         ORDER BY id DESC
         LIMIT ?"
    )
    .bind(limit)
    .fetch_all(&*pool).await
    .map_err(|e| format!("查询会话失败: {}", e))?;

    Ok(rows.into_iter().map(|(id, ts, total, correct, tags)| SessionRecord {
        id,
        started_at: ts,
        total: total as i64,
        correct: correct as i64,
        tags: tags.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
    }).collect())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 加载配置文件（API Key 等）
            let config_dir = app
                .path()
                .app_config_dir()
                .expect("无法获取应用配置目录");
            let cfg = AppConfig::load(&config_dir);
            app.manage(Mutex::new(cfg));
            app.manage(ConfigDir(config_dir));

            // 解析数据库路径：放在 app_data_dir，避免 dev 监视器把 SQLite WAL 写入误判为源码变更
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("无法获取应用数据目录");
            let _ = std::fs::create_dir_all(&app_data_dir);
            let db_path = app_data_dir.join("forgerust.db");

            // 一次性迁移：若旧位置（src-tauri/forgerust.db 或 cwd/forgerust.db）有数据，复制到新位置
            if !db_path.exists() {
                for legacy in ["forgerust.db", "src-tauri/forgerust.db", "../forgerust.db"] {
                    let p = PathBuf::from(legacy);
                    if p.exists() {
                        if let Err(e) = std::fs::copy(&p, &db_path) {
                            eprintln!("迁移旧数据库失败: {}", e);
                        } else {
                            println!("已迁移旧数据库 {} -> {}", p.display(), db_path.display());
                        }
                        break;
                    }
                }
            }

            println!("数据库位置: {}", db_path.display());

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match db::init_db(db_path).await {
                    Ok(pool) => {
                        handle.manage(pool);
                        println!("数据库连接池挂载成功！");
                    }
                    Err(e) => eprintln!("数据库初始化失败: {}", e),
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_random_question,
            generate_interview,
            generate_interview_from_ids,
            list_topics,
            create_topic,
            start_mock_interview,
            submit_mock_answer,
            submit_mock_follow_up,
            record_skipped_question,
            finish_mock_interview,
            evaluate_answer,
            import_questions_from_file,
            get_all_tags,
            get_tag_counts,
            get_api_config,
            set_api_config,
            save_training_session,
            get_wrong_questions,
            remove_from_wrong_book,
            generate_questions_by_ai,
            save_ai_generated_questions,
            get_dashboard_stats,
            get_accuracy_trend,
            get_tag_mastery,
            get_recent_sessions,
            delete_session,
            list_questions,
            delete_question,
            count_questions,
            create_question,
            update_question,
            export_questions,
            mark_question_wrong,
            parse_resume,
            start_interview,
            interview_respond,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

const PHASE_DONE_MARK: &str = "[PHASE_DONE]";

// 剥离面试官输出里的 [PHASE_DONE] 标记，返回 (清理后的文本, 是否检测到标记)
fn strip_phase_done(text: &str) -> (String, bool) {
    if let Some(idx) = text.find(PHASE_DONE_MARK) {
        let mut cleaned = String::with_capacity(text.len());
        cleaned.push_str(&text[..idx]);
        cleaned.push_str(&text[idx + PHASE_DONE_MARK.len()..]);
        (cleaned.trim().to_string(), true)
    } else {
        (text.trim().to_string(), false)
    }
}

// 决定下一次面试官提问所属环节，以及面试是否应结束。
// 入参：当前 phase、project 已问轮数、project 上限、fundamental 已问轮数、fundamental 上限、本轮是否收到 PHASE_DONE。
// 返回：(下一环节, 是否结束)
fn decide_phase(
    current_phase: &str,
    project_used: i32,
    project_cap: i32,
    fundamental_used: i32,
    fundamental_cap: i32,
    phase_done: bool,
) -> (String, bool) {
    if current_phase == "project" {
        if phase_done || project_used >= project_cap {
            // 项目环节结束，转八股；若八股上限为 0 则直接结束
            if fundamental_cap <= 0 {
                return ("fundamental".into(), true);
            }
            return ("fundamental".into(), false);
        }
        ("project".into(), false)
    } else {
        // fundamental 环节
        if phase_done || fundamental_used >= fundamental_cap {
            return ("fundamental".into(), true);
        }
        ("fundamental".into(), false)
    }
}

#[cfg(test)]
mod phase_tests {
    use super::*;

    #[test]
    fn strip_phase_done_detects_and_removes_marker() {
        let (text, done) = strip_phase_done("很好，项目部分聊得差不多了。\n[PHASE_DONE]");
        assert!(done);
        assert!(!text.contains("[PHASE_DONE]"));
        assert!(text.contains("项目部分"));

        let (text2, done2) = strip_phase_done("继续说说你的下一个项目？");
        assert!(!done2);
        assert_eq!(text2, "继续说说你的下一个项目？");
    }

    #[test]
    fn decide_phase_advances_on_cap() {
        // project 环节，已问满 cap → 切 fundamental
        assert_eq!(decide_phase("project", 5, 5, 0, 5, false).0, "fundamental");
        // project 环节，未满且无 PHASE_DONE → 留 project
        assert_eq!(decide_phase("project", 2, 5, 0, 5, false).0, "project");
        // project 环节，收到 PHASE_DONE → 切 fundamental
        assert_eq!(decide_phase("project", 2, 5, 0, 5, true).0, "fundamental");
    }

    #[test]
    fn decide_phase_finishes_when_fundamental_full() {
        // fundamental 环节问满 cap → finished=true
        let (_phase, finished) = decide_phase("fundamental", 5, 5, 5, 5, false);
        assert!(finished);
        // fundamental 未满 → 不结束
        let (_p, fin2) = decide_phase("fundamental", 5, 5, 2, 5, false);
        assert!(!fin2);
    }
}
