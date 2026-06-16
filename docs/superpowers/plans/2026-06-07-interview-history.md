# 面试记录（历史回顾）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 新增一个独立页面「面试记录」，列出已完成的模拟面试，可查看单场复盘（雷达图 + 总结 + 对话回放）并删除记录。

**Architecture:** 后端加只读查询 + 删除命令（数据已落库，详情不重新调 LLM）。前端加一个 `View` 与侧边栏项，新组件 `InterviewHistory.vue` 用 list/detail 两态，复用模拟面试 report 阶段的雷达图与气泡视觉。

**Tech Stack:** Rust + Tauri 2 + sqlx(SQLite) + Vue 3 `<script setup>` + TypeScript + ECharts。

**约束：代码精简**——沿用现有命令/导航/样式 token 模式，复用 `InterviewReport2`/`DimensionScores` 模型。

---

## 文件结构

- 修改 `src-tauri/src/models.rs`：新增 `InterviewSummary`。
- 修改 `src-tauri/src/db.rs`：新增 `list_finished_interviews` / `get_interview_meta` / `delete_interview_cascade` + 单测。
- 修改 `src-tauri/src/lib.rs`：新增 3 个命令并注册。
- 修改 `src/components/layout/Sidebar.vue`：`View` 加项 + 导航项。
- 修改 `src/components/layout/AppShell.vue`：接线新页面。
- 新建 `src/components/InterviewHistory.vue`：列表/详情两态组件。

---

## Task 1: 后端模型 + DB 辅助函数 + 测试

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/db.rs`

- [ ] **Step 1: 新增 InterviewSummary 模型**

在 `models.rs` 末尾追加：

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

- [ ] **Step 2: 写失败测试**

在 `db.rs` 的 `mod tests` 内追加（沿用 `test_db_path` + `pool.close()` + `remove_file` 模式；用已有的 `finish_interview2` 把面试置为 finished）：

```rust
#[tokio::test]
async fn list_and_delete_interview_roundtrip() {
    let db_path = test_db_path("history");
    let pool = init_db(db_path.clone()).await.unwrap();

    let resume_id = create_resume(&pool, "txt", "王五", "[]", r#"["Go"]"#).await.unwrap();
    let iv_id = create_interview2(&pool, resume_id, 5, 5, "Go").await.unwrap();
    add_interview_message(&pool, iv_id, "interviewer", "project", "介绍项目？").await.unwrap();
    add_interview_message(&pool, iv_id, "candidate", "project", "我做了X").await.unwrap();
    // 置为已完成并写入分数
    finish_interview2(&pool, iv_id, 80.0, r#"{"project_depth":85,"fundamental_solidity":75,"communication":80}"#, "总体不错").await.unwrap();

    // 列表只含已完成的
    let rows = list_finished_interviews(&pool).await.unwrap();
    assert_eq!(rows.len(), 1);
    let (id, _created, candidate, tags, avg, dim_json) = &rows[0];
    assert_eq!(*id, iv_id);
    assert_eq!(candidate, "王五");
    assert_eq!(tags, "Go");
    assert!((*avg - 80.0).abs() < 1e-6);
    assert!(dim_json.contains("project_depth"));

    // 元信息
    let (avg2, dim2, summary) = get_interview_meta(&pool, iv_id).await.unwrap();
    assert!((avg2 - 80.0).abs() < 1e-6);
    assert!(dim2.contains("85"));
    assert_eq!(summary, "总体不错");

    // 删除：两表都清空
    delete_interview_cascade(&pool, iv_id).await.unwrap();
    assert_eq!(list_finished_interviews(&pool).await.unwrap().len(), 0);
    let msg_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM interview_messages WHERE interview_id = ?")
        .bind(iv_id).fetch_one(&pool).await.unwrap();
    assert_eq!(msg_count, 0);

    pool.close().await;
    let _ = std::fs::remove_file(db_path);
}
```

- [ ] **Step 3: 运行测试确认失败**

Run: `cd src-tauri && cargo test list_and_delete_interview_roundtrip`
Expected: 编译失败——三个函数未定义。

- [ ] **Step 4: 实现三个 db 辅助函数**

在 `db.rs`（`finish_interview2` 之后、`mod tests` 之前）新增：

```rust
// 已完成面试列表：返回 (id, created_at, candidate, tags, average_score, dimension_scores_json)
pub async fn list_finished_interviews(
    pool: &SqlitePool,
) -> Result<Vec<(i64, String, String, String, f64, String)>, String> {
    sqlx::query_as::<_, (i64, String, String, String, f64, String)>(
        "SELECT mi.id, mi.created_at, COALESCE(r.candidate, ''), mi.tags, mi.average_score, mi.dimension_scores
         FROM mock_interviews mi
         LEFT JOIN resumes r ON mi.resume_id = r.id
         WHERE mi.status = 'finished'
         ORDER BY mi.id DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("读取面试记录失败: {}", e))
}

// 单场面试元信息：(average_score, dimension_scores_json, summary)
pub async fn get_interview_meta(pool: &SqlitePool, interview_id: i64) -> Result<(f64, String, String), String> {
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
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cd src-tauri && cargo test list_and_delete_interview_roundtrip`
Expected: PASS。再跑 `cargo test` 确认既有全绿。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/models.rs src-tauri/src/db.rs
git commit -m "feat: db 层新增面试记录列表/详情/删除辅助函数与测试"
```

---

## Task 2: 后端命令 + 注册

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: import 补充 InterviewSummary**

在 `lib.rs` 顶部 `use crate::models::{...}` 列表中加入 `InterviewSummary`（与 `InterviewReport2`、`DimensionScores` 同处，按现有多行风格补一个名字）。

- [ ] **Step 2: 实现 3 个命令**

在 `lib.rs` 对话式面试命令区（`finish_interview` 之后）新增：

```rust
#[tauri::command]
async fn list_interviews(pool: tauri::State<'_, SqlitePool>) -> Result<Vec<InterviewSummary>, String> {
    let rows = db::list_finished_interviews(&pool).await?;
    Ok(rows
        .into_iter()
        .map(|(id, created_at, candidate, tags, average_score, dim_json)| {
            let dimension_scores: DimensionScores =
                serde_json::from_str(&dim_json).unwrap_or(DimensionScores {
                    project_depth: 0,
                    fundamental_solidity: 0,
                    communication: 0,
                });
            InterviewSummary { id, created_at, candidate, tags, average_score, dimension_scores }
        })
        .collect())
}

#[tauri::command]
async fn get_interview_detail(
    interview_id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<InterviewReport2, String> {
    let (average_score, dim_json, summary) = db::get_interview_meta(&pool, interview_id).await?;
    let dimension_scores: DimensionScores =
        serde_json::from_str(&dim_json).unwrap_or(DimensionScores {
            project_depth: 0,
            fundamental_solidity: 0,
            communication: 0,
        });
    let messages = db::get_interview_messages(&pool, interview_id).await?;
    Ok(InterviewReport2 { interview_id, average_score, dimension_scores, summary, messages })
}

#[tauri::command]
async fn delete_interview(
    interview_id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    db::delete_interview_cascade(&pool, interview_id).await
}
```

- [ ] **Step 3: 注册命令**

在 `tauri::generate_handler![` 列表中（`finish_interview,` 之后）加入：

```rust
            list_interviews,
            get_interview_detail,
            delete_interview,
```

- [ ] **Step 4: 编译 + 测试**

Run: `cd src-tauri && cargo build && cargo test`
Expected: 编译通过、无警告；测试全绿。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: 新增面试记录列表/详情/删除命令并注册"
```

---

## Task 3: 侧边栏导航 + AppShell 接线

**Files:**
- Modify: `src/components/layout/Sidebar.vue`
- Modify: `src/components/layout/AppShell.vue`

- [ ] **Step 1: Sidebar 加 View 与导航项**

在 `Sidebar.vue` 的 `View` 联合类型加入 `"interview_history"`：

```ts
export type View =
  | "dashboard"
  | "training"
  | "wrong_book"
  | "ai_generate"
  | "question_library"
  | "mock_interview"
  | "interview_history"
  | "settings";
```

在 `primary` 数组「模拟面试」那项之后加入：

```ts
  { view: "interview_history", label: "面试记录", icon: "History" },
```

- [ ] **Step 2: AppShell 接线**

在 `AppShell.vue` 的 import 区（`import MockInterview ...` 之后）加：

```ts
import InterviewHistory from "../InterviewHistory.vue";
```

在 `<main>` 内 `<MockInterview ... />` 之后加：

```html
      <InterviewHistory
        v-show="currentView === 'interview_history'"
        :is-active="currentView === 'interview_history'"
      />
```

- [ ] **Step 3: 构建验证**

Run: `cd "e:/2026_Junior_S2/Rust/Project/ForgeRust" && npm run build`
Expected: 因 `InterviewHistory.vue` 尚未创建会报找不到模块——本步先确认 Sidebar 的类型改动无误即可；模块缺失错误在 Task 4 创建组件后消除。（若想本步即绿，可先创建空壳组件，但推荐直接进入 Task 4。）

- [ ] **Step 4: Commit**

```bash
git add src/components/layout/Sidebar.vue src/components/layout/AppShell.vue
git commit -m "feat: 侧边栏新增「面试记录」入口并接线"
```

---

## Task 4: InterviewHistory.vue 组件（列表 + 详情）

**Files:**
- Create: `src/components/InterviewHistory.vue`

- [ ] **Step 1: 创建组件**

创建 `src/components/InterviewHistory.vue`：

```vue
<script setup lang="ts">
import { ref, watch, shallowRef, nextTick, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Icon from "./ui/Icon.vue";

import * as echarts from "echarts/core";
import { RadarChart } from "echarts/charts";
import { TooltipComponent } from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
echarts.use([RadarChart, TooltipComponent, CanvasRenderer]);

interface DimensionScores { project_depth: number; fundamental_solidity: number; communication: number; }
interface InterviewSummary { id: number; created_at: string; candidate: string; tags: string; average_score: number; dimension_scores: DimensionScores; }
interface InterviewReport {
  interview_id: number;
  average_score: number;
  dimension_scores: DimensionScores;
  summary: string;
  messages: { role: string; phase: string; content: string; seq: number }[];
}

const props = defineProps<{ isActive: boolean }>();

const view = ref<"list" | "detail">("list");
const loading = ref(false);
const errorMsg = ref("");
const items = ref<InterviewSummary[]>([]);
const confirmingId = ref<number | null>(null);

const report = ref<InterviewReport | null>(null);
const radarEl = ref<HTMLElement | null>(null);
const radarChart = shallowRef<echarts.ECharts | null>(null);

watch(
  () => props.isActive,
  (active) => {
    if (active && view.value === "list") loadList();
  },
  { immediate: true }
);

async function loadList() {
  loading.value = true;
  errorMsg.value = "";
  try {
    items.value = await invoke<InterviewSummary[]>("list_interviews");
  } catch (err) {
    errorMsg.value = String(err);
  } finally {
    loading.value = false;
  }
}

async function openDetail(id: number) {
  loading.value = true;
  errorMsg.value = "";
  try {
    report.value = await invoke<InterviewReport>("get_interview_detail", { interviewId: id });
    view.value = "detail";
    nextTick(renderRadar);
  } catch (err) {
    errorMsg.value = String(err);
  } finally {
    loading.value = false;
  }
}

function backToList() {
  view.value = "list";
  radarChart.value?.dispose();
  radarChart.value = null;
  report.value = null;
}

function requestDelete(id: number) { confirmingId.value = id; }
function cancelDelete() { confirmingId.value = null; }
async function confirmDelete(id: number) {
  try {
    await invoke("delete_interview", { interviewId: id });
    items.value = items.value.filter((it) => it.id !== id);
  } catch (err) {
    errorMsg.value = String(err);
  } finally {
    confirmingId.value = null;
  }
}

function renderRadar() {
  if (!radarEl.value || !report.value) return;
  if (!radarChart.value) radarChart.value = echarts.init(radarEl.value);
  const d = report.value.dimension_scores;
  radarChart.value.setOption({
    tooltip: {},
    radar: {
      indicator: [
        { name: "项目深度", max: 100 },
        { name: "八股扎实度", max: 100 },
        { name: "表达逻辑", max: 100 },
      ],
      radius: "65%",
    },
    series: [{
      type: "radar",
      data: [{ value: [d.project_depth, d.fundamental_solidity, d.communication], name: "本场表现" }],
      areaStyle: { opacity: 0.2 },
    }],
  });
}

onUnmounted(() => { radarChart.value?.dispose(); });
</script>

<template>
  <div class="fr-page history-page">
    <header class="hist-head">
      <div>
        <h1 class="fr-page-title">面试记录</h1>
        <p class="fr-page-subtitle">回顾已完成的模拟面试与多维复盘。</p>
      </div>
      <button v-if="view === 'detail'" class="fr-btn fr-btn-ghost" @click="backToList">
        <Icon name="ArrowLeft" :size="14" /><span>返回列表</span>
      </button>
    </header>

    <!-- 列表态 -->
    <section v-if="view === 'list'" class="list-panel">
      <p v-if="loading" class="empty-hint">加载中…</p>
      <p v-else-if="!items.length" class="empty-hint">还没有已完成的面试记录，去「模拟面试」开始一场吧。</p>
      <ul v-else class="hist-list">
        <li v-for="it in items" :key="it.id" class="hist-row fr-card">
          <div class="row-main">
            <span class="row-candidate">{{ it.candidate || "未命名候选人" }}</span>
            <span v-if="it.tags" class="row-tags">{{ it.tags }}</span>
            <span class="row-date">{{ it.created_at }}</span>
          </div>
          <span class="row-score">{{ it.average_score.toFixed(1) }}</span>
          <div class="row-actions">
            <template v-if="confirmingId === it.id">
              <span class="confirm-text">确定删除？</span>
              <button class="fr-btn fr-btn-ghost" @click="cancelDelete">取消</button>
              <button class="fr-btn fr-btn-danger" @click="confirmDelete(it.id)">删除</button>
            </template>
            <template v-else>
              <button class="fr-btn fr-btn-ghost" @click="openDetail(it.id)"><Icon name="Eye" :size="14" /><span>查看</span></button>
              <button class="icon-btn danger" @click="requestDelete(it.id)"><Icon name="Trash2" :size="15" /></button>
            </template>
          </div>
        </li>
      </ul>
      <p v-if="errorMsg" class="error-msg">{{ errorMsg }}</p>
    </section>

    <!-- 详情态 -->
    <section v-else-if="report" class="detail-panel">
      <div class="fr-card report-hero">
        <div>
          <span class="report-label">面试复盘</span>
          <h2>{{ report.average_score.toFixed(1) }}</h2>
          <p>{{ report.summary }}</p>
        </div>
      </div>

      <div class="fr-card dim-card">
        <h3>能力维度</h3>
        <div ref="radarEl" class="radar"></div>
      </div>

      <div class="transcript-replay">
        <div v-for="(m, i) in report.messages" :key="i" :class="['bubble-row', m.role === 'interviewer' ? 'interviewer' : 'candidate']">
          <div class="bubble">{{ m.content }}</div>
        </div>
      </div>
      <p v-if="errorMsg" class="error-msg">{{ errorMsg }}</p>
    </section>
  </div>
</template>

<style scoped>
.history-page { max-width: var(--content-max); margin: 0 auto; display: flex; flex-direction: column; gap: var(--sp-4); }
.hist-head { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--sp-4); }
.list-panel, .detail-panel { display: flex; flex-direction: column; gap: var(--sp-3); }
.empty-hint { font-size: var(--fs-13); color: var(--text-subtle); padding: var(--sp-6); text-align: center; }
.error-msg { color: var(--danger); font-size: var(--fs-13); }

.hist-list { display: flex; flex-direction: column; gap: var(--sp-2); }
.hist-row { display: flex; align-items: center; gap: var(--sp-4); padding: var(--sp-3) var(--sp-4); }
.row-main { flex: 1; display: flex; align-items: center; gap: var(--sp-3); min-width: 0; }
.row-candidate { font-size: var(--fs-14); font-weight: var(--fw-medium); color: var(--text); }
.row-tags { font-size: var(--fs-12); color: var(--accent); background: var(--accent-soft); border-radius: 999px; padding: 2px 8px; }
.row-date { font-size: var(--fs-12); color: var(--text-subtle); }
.row-score { font-size: var(--fs-20); font-family: var(--font-mono); color: var(--accent); }
.row-actions { display: flex; align-items: center; gap: var(--sp-2); }
.confirm-text { font-size: var(--fs-12); color: var(--text-muted); }
.icon-btn { width: 30px; height: 30px; border-radius: var(--radius-sm); color: var(--text-subtle); display: inline-flex; align-items: center; justify-content: center; }
.icon-btn.danger:hover { color: var(--danger); background: var(--danger-soft); }

.report-hero { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--sp-4); }
.report-label { font-size: var(--fs-12); color: var(--text-muted); }
.report-hero h2 { font-size: var(--fs-28); color: var(--accent); margin: var(--sp-1) 0; }
.report-hero p { color: var(--text-muted); line-height: 1.6; }
.dim-card { display: flex; flex-direction: column; gap: var(--sp-3); }
.dim-card h3 { font-size: var(--fs-14); font-weight: var(--fw-semibold); }
.radar { width: 100%; height: 280px; }

.transcript-replay { display: flex; flex-direction: column; gap: var(--sp-2); }
.bubble-row { display: flex; }
.bubble-row.interviewer { justify-content: flex-start; }
.bubble-row.candidate { justify-content: flex-end; }
.bubble { max-width: 78%; padding: 8px 12px; font-size: var(--fs-13); line-height: 1.6; border-radius: var(--radius-lg); }
.bubble-row.interviewer .bubble { background: var(--accent-soft); color: var(--text); border-bottom-left-radius: var(--radius-sm); }
.bubble-row.candidate .bubble { background: var(--surface-2); color: var(--text); border: 1px solid var(--border); border-bottom-right-radius: var(--radius-sm); }
</style>
```

- [ ] **Step 2: 构建验证**

Run: `cd "e:/2026_Junior_S2/Rust/Project/ForgeRust" && npm run build`
Expected: vue-tsc + Vite 构建无错误（Task 3 的模块缺失错误此时消除）。

- [ ] **Step 3: Commit**

```bash
git add src/components/InterviewHistory.vue
git commit -m "feat: 新增面试记录页（列表 + 雷达复盘详情 + 删除）"
```

---

## 完成标准

- `cargo build && cargo test` 全绿、无警告（新增 1 个 db roundtrip 测试）。
- `npm run build` 无错误。
- 侧边栏出现「面试记录」；进入后列出已完成面试（候选人/标签/日期/均分），可查看（雷达 + 总结 + 对话回放）与删除（带确认）。
- ⚠️ GUI 交互由用户 `npm run tauri dev` 实测。

---

## 自检备注（写计划时已核对）

- 命令参数 camelCase：`get_interview_detail({interviewId})`、`delete_interview({interviewId})`；`list_interviews` 无参。与后端 snake_case 形参经 Tauri 自动映射对应。
- 复用模型：`get_interview_detail` 返回 `InterviewReport2`（已有，含 `messages: Vec<InterviewMessage>`），前端 `InterviewReport` interface 字段名（snake_case：interview_id/average_score/dimension_scores/summary/messages）与之对齐。
- `list_finished_interviews` 返回 6 元组类型 `(i64,String,String,String,f64,String)`，与 Task 2 命令层解构一致。
- 雷达图配置与 MockInterview report 阶段完全一致，视觉统一。
- 详情纯读 DB，无 llm_client 调用。
