# Question Dedup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为题库新增“标准化 hash 去重 + 疑似重复标记”，降低导入、AI 生成、手动录入时出现重复题的概率。

**Architecture:** 后端在写入题目前对题干做统一标准化，生成稳定 `content_hash`；若发现已有相同 hash 的题目，新题保留但标记为 `needs_review` 并记录 `duplicate_of`，让用户审核合并。前端在题库列表和详情弹窗中展示疑似重复线索。

**Tech Stack:** Rust, sqlx, SQLite, Tauri commands, Vue 3.

---

### Task 1: 后端回归测试

**Files:**
- Modify: `src-tauri/src/db.rs`

- [ ] 增加测试 `normalize_question_content_collapses_spacing_punctuation_and_case`，验证空格、常见中英文标点、大小写不会影响 hash。
- [ ] 增加测试 `create_question_marks_normalized_duplicate_for_review`，先插入原题，再插入标准化后相同的变体题，期望第二题 `duplicate_of` 指向原题、`quality_status` 为 `needs_review`、`quality_note` 包含“疑似重复”。
- [ ] 先运行 `cargo test normalize_question_content_collapses_spacing_punctuation_and_case create_question_marks_normalized_duplicate_for_review`，预期失败。

### Task 2: 数据模型与迁移

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/db.rs`

- [ ] 在 `Question` 增加 `content_hash: String` 与 `duplicate_of: Option<i32>`。
- [ ] 在 `questions` 表新增 `content_hash TEXT NOT NULL DEFAULT ''` 与 `duplicate_of INTEGER`。
- [ ] `init_db` 对老库执行 `ALTER TABLE`，并为旧题按题干回填 hash。

### Task 3: 去重核心逻辑

**Files:**
- Modify: `src-tauri/src/db.rs`

- [ ] 新增 `normalize_question_content`：保留 ASCII 字母数字和 CJK 字符，统一小写，移除空白和标点。
- [ ] 新增 `question_content_hash`：对标准化文本生成稳定 FNV-1a 64-bit 十六进制 hash。
- [ ] 新增 `prepare_question_dedup_fields`：查找同 hash 旧题，自动输出最终 `quality_status`、`quality_note`、`duplicate_of`。
- [ ] `create_question` 和 `update_question` 写入/更新 `content_hash` 与 `duplicate_of`。

### Task 4: 导入与 AI 保存链路

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] `import_questions_from_file` 写入前调用去重辅助函数，并把 `content_hash`、`duplicate_of` 带入 upsert。
- [ ] `save_ai_generated_questions` 同样接入去重辅助函数。
- [ ] exact content 冲突更新时排除自身，避免 `duplicate_of` 指向自己。

### Task 5: 前端可见性

**Files:**
- Modify: `src/components/QuestionLibrary.vue`
- Modify: `src/components/ui/QuestionModal.vue`

- [ ] 题库列表对 `duplicate_of` 显示“疑似重复 #ID”提示。
- [ ] 详情弹窗展示只读重复线索，方便答辩演示题库质量控制。

### Task 6: 验证

**Files:**
- Test command: `cargo test`
- Build command: `npm run build`

- [ ] 运行 `cargo test`，确认后端回归通过。
- [ ] 运行 `npm run build`，确认前端类型和打包通过。
- [ ] 用题库新增两条“TCP三次握手的目的是什么”变体题，确认第二条显示“需复核 / 疑似重复”。
