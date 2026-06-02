# ForgeRust 设计文档：模拟面试重构 + 题库 CRUD 完整化

- 日期：2026-06-03
- 范围：两个相对独立的阶段。阶段一为题库 CRUD 完整化与小优化；阶段二为模拟面试从「随机抽题」重构为「全程 LLM 驱动的对话式面试」。
- 全局约束：**代码尽量精简**——优先复用现有组件与样式、拆小可复用单元、不堆冗余逻辑。沿用现有设计语言，避免通用化的「AI 风」样式。

---

## 背景：现有模拟面试为什么是「随机跳题」

读现有代码（`MockInterview.vue`、`lib.rs`、`llm_client.rs`、`db.rs`）后确认根因：

1. `start_mock_interview` 一次性 `ORDER BY RANDOM()` 抽 N 题，全程走完这 N 题。
2. `evaluate_mock_interview_answer` 每次调用**无状态**——只传「当前题 + 标准答案 + 当前回答」，不带对话历史，AI 不知道前面聊过什么。
3. `submit_mock_follow_up` 只存追问回答文本，**从不评分、从不继续追问**——追问是死胡同。
4. 没有简历、没有环节概念，题目来源仅靠题库标签筛选。

目标：换成「LLM 作为有记忆的面试官」的对话模式。

---

## 阶段二：模拟面试重构（核心）

### 已确认的关键决策

| 维度 | 决策 |
|------|------|
| 简历输入 | PDF 上传，前端用 `pdfjs-dist` 在 webview 抽纯文本 |
| 面试引擎 | 全程 LLM 驱动，有记忆的对话循环 |
| 八股来源 | 纯 LLM 按简历技术栈现场出题，不依赖题库 |
| 节奏控制 | 项目/八股各设轮数上限；后端确定性计数 + LLM 可提前切换（方案 A）|
| 评分 | 面试结束时一次性多维度总评 |
| 输出呈现 | 真流式 SSE 逐字（Tauri events） |

### 架构总览

```
PDF → (前端 pdfjs-dist 抽文本) → parse_resume → LLM 结构化解析
        → { candidate, projects[], tech_stack[] } 存 resumes 表
                              │
                              ▼
   start_interview(resume_id, project_cap, fundamental_cap)
                              │
        ┌─────────────────────┴──────────────────────┐
        │  对话循环（每轮一次流式 LLM 调用）              │
        │  后端状态机: phase(project/fundamental) + 已用轮数 │
        │  系统提示词注入: 简历摘要 + 当前环节 + 剩余轮数指令  │
        │  历史消息全量传入 → 流式吐出面试官下一句           │
        └─────────────────────┬──────────────────────┘
                              ▼
            轮数耗尽 → finish_interview → 多维度总评报告
```

### 流式与控制信号的共存（方案 A）

矛盾：SSE 流式适合吐纯文本，但后端还需结构化控制信号（当前环节、是否提前切换/结束）。

采用方案 A：**后端确定性控制 + LLM 只管说话**。
- 轮数上限由后端计数器严格执行：项目环节满 `project_cap` 自动切八股；八股满 `fundamental_cap` 标记 `finished`。
- 「LLM 提前切换」：LLM 在回复**首行**输出控制标记 `[PHASE_DONE]`，后端识别后剥离、不下发前端，标记本环节提前结束；其余正文照常流式。
- 标记解析需容错（首行 trim 后精确匹配 `[PHASE_DONE]`，匹配不到则当普通正文）。

理由：流式纯净、状态机可靠、每轮仅一次 LLM 调用（成本最低），同时保留 LLM 自主推进的灵活性。

### 数据库 Schema 改动

老 `mock_interview_turns` 表**保留不动**（旧数据兼容），新流程不写它。

新增表 `resumes`：
```sql
CREATE TABLE resumes (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now','localtime')),
    raw_text    TEXT NOT NULL,
    candidate   TEXT NOT NULL DEFAULT '',
    projects    TEXT NOT NULL DEFAULT '[]', -- JSON: [{name, role, summary, highlights[]}]
    tech_stack  TEXT NOT NULL DEFAULT '[]'  -- JSON: ["Java","Rust","MySQL"...]
);
```

改造表 `mock_interviews`（增列，不破坏旧列）：
```sql
ALTER TABLE mock_interviews ADD COLUMN resume_id        INTEGER;
ALTER TABLE mock_interviews ADD COLUMN project_cap      INTEGER NOT NULL DEFAULT 5;
ALTER TABLE mock_interviews ADD COLUMN fundamental_cap  INTEGER NOT NULL DEFAULT 5;
ALTER TABLE mock_interviews ADD COLUMN dimension_scores TEXT NOT NULL DEFAULT '{}';
-- 沿用已有列: average_score(总分), summary(文字复盘), status, tags, created_at, ended_at
```

新增表 `interview_messages`（对话消息，取代 turns 概念）：
```sql
CREATE TABLE interview_messages (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    interview_id INTEGER NOT NULL REFERENCES mock_interviews(id) ON DELETE CASCADE,
    role         TEXT NOT NULL,   -- 'interviewer' | 'candidate'
    phase        TEXT NOT NULL,   -- 'project' | 'fundamental'
    content      TEXT NOT NULL,
    seq          INTEGER NOT NULL,
    created_at   TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);
```

多维度分（`dimension_scores` 存 JSON，前端直接渲染，不固定列）。初版三维：
```json
{ "project_depth": 82, "fundamental_solidity": 75, "communication": 88 }
```
对应中文：项目深度 / 八股扎实度 / 表达逻辑。`average_score` 取三项均值作为总分。

> 注：`ALTER TABLE ... ADD COLUMN` 在已存在列时会报错，沿用现有 `db.rs` 的写法（`let _ = sqlx::query(...).await;` 忽略错误）实现幂等迁移。

### 后端命令（Tauri）

| 命令 | 作用 | 流式 |
|------|------|------|
| `parse_resume(raw_text)` | LLM 解析为 `{candidate, projects[], tech_stack[]}`，存 `resumes`，返回 resume_id + 结果 | 否 |
| `start_interview(resume_id, project_cap, fundamental_cap)` | 建面试记录，流式生成开场白 + 第一个项目问题 | 是 |
| `interview_respond(interview_id, answer)` | 存候选人回答 → 推进状态机 → 流式生成面试官下一句 | 是 |
| `finish_interview(interview_id)` | 全量对话 → LLM 多维度总评，写回 `mock_interviews` | 否 |

旧命令 `start_mock_interview / submit_mock_answer / submit_mock_follow_up / record_skipped_question / finish_mock_interview` 从新前端流程移除调用；后端代码可保留或删除（实施时按精简原则决定，倾向删除未被引用的）。

### 流式机制（Tauri events）

`start_interview` / `interview_respond` 内部 `tokio::spawn`，通过事件推送：
- `interview-token` → `{ interview_id, chunk }`：逐块文本
- `interview-turn-done` → `{ interview_id, phase, finished }`：本轮结束，带当前环节 + 是否已到可结束状态

前端监听这两个事件做逐字渲染。

### 状态机落地

后端每场面试持有 `project_used / fundamental_used` 计数。每轮：
1. 组装 system prompt：注入简历摘要 + 当前环节 + 剩余轮数 + 指令（项目环节追问项目细节/技术选型；八股环节按技术栈考原理）。
2. 历史消息全量传入（`interview_messages` 按 `seq` 取出）。
3. 流式读取 LLM 输出：首行若为 `[PHASE_DONE]` 则剥离、标记本环节提前结束；其余正文流式下发并落库。
4. 更新计数器与 `phase`：达 `project_cap` 或收到 `[PHASE_DONE]` → 切八股；八股同理 → 标记 `finished`。

### LLM 客户端改动（`llm_client.rs`）

- 新增 `call_api_stream(...)`：用 `reqwest` 的 `bytes_stream()` 读 SSE，逐块回调（现有 `call_api` 保留给非流式命令）。
- 新增 `parse_resume_llm(...)`、`evaluate_interview(...)`（多维度 JSON 输出）。
- 修 bug：把硬编码的 `"model": "deepseek-chat"` 提到 config 可配置，流式/非流式共用（与阶段一的「模型名可配置」同源，提前到阶段一完成）。

### 前端流程与 UI（`MockInterview.vue`）

stage 调整为 `resume → interview → report`，沿用现有视频面试风设计语言。

- **Stage `resume`**：上传 PDF → `pdfjs-dist` 抽文本 → `parse_resume` → 展示解析卡片（项目列表 + 技术栈标签，可删错识别项）→ 设两个环节轮数上限 → 开始。无 API Key 复用现有提示。
- **Stage `interview`**：保留视频条 + 计时器；新增环节徽章（项目/八股，随 `interview-turn-done` 切换）；对话气泡复用现有 `transcript`，面试官气泡接 `interview-token` 真流式渲染（删除假打字机 `typePrompt`）；作答区简化为单一回答框（删除主回答/追问双模式 `currentInput`）；按钮：回答 / 跳过 / 退出。
- **Stage `report`**：顶部总分 + 文字复盘（复用现有 hero）；新增 ECharts **雷达图**展示三维分；完整对话回放（按 `interview_messages` 顺序，分环节）。

代码影响：现有 `submitAnswer/nextQuestion/skipQuestion`、`currentEvaluation` 追问态、假打字机全部重写为事件驱动对话流。**拆出 `useInterviewStream.ts` composable** 专管事件监听与流式拼接，组件聚焦 UI（符合精简原则）。

新增前端依赖：`pdfjs-dist`。

---

## 阶段一：题库 CRUD 完整化 + 小优化

独立、风险低，先于阶段二完成；其中「模型名可配置」与阶段二 LLM 改动同源，提前到本阶段。

| 项 | 后端 | 前端 |
|----|------|------|
| 题目编辑 | `update_question(id, ...)` | 编辑弹窗组件 |
| 手动新增单题 | `create_question(...)` | 复用同一弹窗（新增/编辑共用） |
| 题目详情预览 | 复用现有查询 | 复用弹窗只读态 |
| 错题本手动标记 | 标记命令（写 training_records.manually_added 或等效） | 题库/训练页「加入错题本」按钮 |
| 题库导出 | `export_questions` 写 JSON 文件 | 触发按钮 |
| 模型名可配置 | config 增 model 字段 | 设置页输入框 |
| 提交待提交改动 | — | 核对 AIGenerate/Settings/MockInterview/global.css 后提交 |

核心可复用件：**题目弹窗组件（新增/编辑/详情三态合一）**，符合精简原则。

---

## 实施顺序

1. **阶段一**（独立、低风险，顺手修模型名 bug、理清待提交改动）。
2. **阶段二**（大改造，专注推进）。

每阶段各写一份独立的实施计划（plan）。

---

## 单元划分（便于隔离与测试）

- `resumes` 解析（`parse_resume` + `parse_resume_llm`）：输入简历文本，输出结构化 JSON，可独立测试。
- 流式客户端 `call_api_stream`：输入请求，回调文本块，独立于面试逻辑。
- 面试状态机（轮数计数 + phase 切换 + `[PHASE_DONE]` 解析）：纯逻辑，可单元测试。
- `useInterviewStream.ts`：前端事件监听与文本拼接，独立于组件渲染。
- 题目弹窗组件：三态合一，独立复用。

---

## 非目标（本次不做）

- 模拟面试历史记录列表页（归属模拟面试模块，后续单独做）。
- 简历多份管理 / 简历编辑器（本次仅一次性解析使用）。
- 摄像头 / 语音输入（视频条为纯视觉占位）。
- 题库社区分享、PDF 报告导出、移动端等。
