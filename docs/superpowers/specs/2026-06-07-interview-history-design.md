# 面试记录（历史面试回顾）设计文档

## 背景与目标

模拟面试结束后会把面试信息（分数、总结、对话）落库到 `mock_interviews` 与 `interview_messages`，但目前没有任何入口回看过往面试。本功能新增一个独立页面「面试记录」，让用户浏览已完成面试的列表、查看单场复盘（雷达图 + 文字总结 + 对话回放），并可删除记录。

**关键约束：**
- 详情**只读已存数据**，不重新调用 LLM（不产生费用、分数不变）。
- 列表**只显示 `status='finished'`** 的面试（中途退出的 `active` 记录不展示）。
- 复用模拟面试 report 阶段已有的雷达图与对话气泡视觉，不重造样式。
- 代码精简：沿用现有命令模式、`View` 导航机制、设计 token。

## 数据来源（均已落库，无需迁移）

`mock_interviews` 表相关列：`id, created_at, tags, average_score, dimension_scores(JSON), summary, status, resume_id`。
`resumes` 表：`id, candidate`。
`interview_messages` 表：`role, phase, content, seq`（按 `interview_id`）。

## 架构

### 后端（src-tauri）

**新增模型**（`models.rs`）：
```rust
#[derive(Debug, Serialize)]
pub struct InterviewSummary {
    pub id: i64,
    pub created_at: String,
    pub candidate: String,
    pub tags: String,
    pub average_score: f64,
    pub dimension_scores: DimensionScores,
}
```

**新增 db 辅助函数**（`db.rs`）：
- `list_finished_interviews(pool) -> Result<Vec<(i64, String, String, String, f64, String)>, String>`
  查询：`SELECT mi.id, mi.created_at, COALESCE(r.candidate,''), mi.tags, mi.average_score, mi.dimension_scores FROM mock_interviews mi LEFT JOIN resumes r ON mi.resume_id = r.id WHERE mi.status='finished' ORDER BY mi.id DESC`。返回原始行（dimension_scores 仍为 JSON 字符串，由命令层解析）。
- `get_interview_meta(pool, id) -> Result<(f64, String, String), String>`
  返回 `(average_score, dimension_scores_json, summary)`，WHERE id=?。
- `delete_interview_cascade(pool, id) -> Result<(), String>`
  先 `DELETE FROM interview_messages WHERE interview_id=?`，再 `DELETE FROM mock_interviews WHERE id=?`（显式两步，不依赖未开启的 FK cascade）。

**新增 Tauri 命令**（`lib.rs`，均注册到 `generate_handler!`）：
- `list_interviews(pool) -> Result<Vec<InterviewSummary>, String>`：调 `list_finished_interviews`，把每行的 `dimension_scores` JSON 解析为 `DimensionScores`（解析失败兜底为全 0），组装 `InterviewSummary`。
- `get_interview_detail(interview_id, pool) -> Result<InterviewReport2, String>`：调 `get_interview_meta` 拿分数/总结、`get_interview_messages` 拿对话，解析 dimension_scores JSON，组装并返回 `InterviewReport2`（复用已有模型）。**不调用 llm_client。**
- `delete_interview(interview_id, pool) -> Result<(), String>`：调 `delete_interview_cascade`。

### 前端（src）

**导航接线：**
- `Sidebar.vue`：`View` 联合类型加 `"interview_history"`；`primary` 导航数组在「模拟面试」后加一项 `{ view: "interview_history", label: "面试记录", icon: "History" }`。
- `AppShell.vue`：import `InterviewHistory`，在 `<main>` 内加 `<InterviewHistory v-show="currentView === 'interview_history'" :is-active="currentView === 'interview_history'" />`。

**新组件 `InterviewHistory.vue`**（两态：`list` / `detail`）：

类型（前端 interface，对齐后端返回）：
```ts
interface InterviewSummary { id: number; created_at: string; candidate: string; tags: string; average_score: number; dimension_scores: { project_depth: number; fundamental_solidity: number; communication: number }; }
interface InterviewReport { interview_id: number; average_score: number; dimension_scores: {...}; summary: string; messages: { role: string; phase: string; content: string; seq: number }[]; }
```

- props：`{ isActive: boolean }`。`watch(isActive)`：变 true 时拉取列表 `list_interviews`（每次进入刷新，保证删除/新面试后同步）。
- **列表态**：
  - 顶部标题「面试记录」+ 副标题。
  - 列表：每行卡片显示 `created_at`、候选人名、`tags`（有才显示）、均分徽章（用 accent 色）；行右侧「查看」与「删除」按钮。
  - 删除：点击 → 行内确认（"确定删除？"+ 取消/确定，复用 MockInterview 退出确认的交互模式），确定后 invoke `delete_interview` 并从本地列表移除。
  - 空状态：`还没有已完成的面试记录，去「模拟面试」开始一场吧。`
- **详情态**：
  - 点「查看」→ invoke `get_interview_detail` → 切到 detail 态。
  - 复用 report 视觉：复盘头（均分大字 + summary）、能力维度雷达图（echarts `RadarChart`，配置照搬 MockInterview）、对话气泡回放（`.bubble-row`/`.bubble` 样式）。
  - 顶部「← 返回列表」按钮回到 list 态；详情态 `onUnmounted`/离开时 `dispose` 雷达图实例。
  - 雷达图渲染时机：切到 detail 且 DOM 就绪后 `nextTick` 渲染（同 MockInterview 做法）。

**样式**：复用 MockInterview report 阶段的 `.radar`、`.bubble*`、`.report-hero`、`.dim-card` 等同款规则（在本组件 `<style scoped>` 内重写一份精简版，保持视觉一致）。

## 数据流

1. 用户点侧边栏「面试记录」→ AppShell 切 `currentView`，`InterviewHistory` 的 `isActive` 变 true → 拉 `list_interviews` → 渲染列表。
2. 点某行「查看」→ `get_interview_detail(id)` → 填充 report → 切 detail 态 → 渲染雷达图 + 气泡。
3. 点「删除」→ 确认 → `delete_interview(id)` → 本地列表移除该项。
4. 「返回列表」→ 切回 list 态（列表已在内存，无需重拉）。

## 错误处理

- 每个 invoke 包 try/catch，失败把错误串显示在页面的 `errorMsg` 区（沿用现有组件错误展示风格）。
- 列表为空 → 空状态文案。
- `dimension_scores` JSON 解析失败 → 兜底三维全 0（不崩）。

## 测试

- 后端：`db.rs` 加单测 `list_and_delete_interview_roundtrip`——init_db → 造一条 finished 面试（create_interview2 + 手动 UPDATE status/average_score/dimension_scores/summary + add_interview_message 若干）→ `list_finished_interviews` 返回 1 条且字段正确 → `get_interview_meta` 正确 → `delete_interview_cascade` 后列表为 0 且 `interview_messages` 也清空。
- 前端：`npm run build`（vue-tsc + vite）通过；GUI 交互由用户实测。

## 不做（YAGNI）

- 不做分页/搜索/筛选（记录量级小，倒序全列即可）。
- 不做重新评分、不做导出。
- 不显示中途放弃（`active`）的面试。
