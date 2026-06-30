# Product Experience Upgrade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add microphone answer input, unify feedback UX, productize mock interview setup, make interview reports actionable, add question quality controls, and do a low-risk cleanup pass.

**Architecture:** Add small frontend composables/components for toast and speech recognition, then wire them into existing Vue pages. Extend the Rust/Tauri data model with additive SQLite migrations for interview options, actionable reports, and question quality metadata. Keep existing command boundaries and tests, avoiding a large rewrite.

**Tech Stack:** Vue 3 + TypeScript + Web Speech API + Tauri 2 + Rust + sqlx(SQLite) + ECharts.

---

## File Structure

- Create `src/composables/useToast.ts`: global toast store and helper functions.
- Create `src/components/ui/ToastHost.vue`: renders toast notifications.
- Create `src/composables/useSpeechRecognition.ts`: Web Speech API wrapper.
- Create `src/utils/interviewOptions.ts`: shared interview option types/defaults/labels.
- Create `src/components/ui/InterviewRadar.vue`: shared ECharts radar chart used by report views.
- Modify `src/components/layout/AppShell.vue`: mount `ToastHost`, pass recommended training tags.
- Modify `src/components/MockInterview.vue`: speech input, interview setup options, actionable report.
- Modify `src/components/InterviewHistory.vue`: show actionable report fields and shared radar.
- Modify `src/components/QuestionLibrary.vue`: replace alerts, show/edit quality metadata.
- Modify `src/components/ui/QuestionModal.vue`: source/review status/quality note form fields.
- Modify `src/components/AIGenerate.vue`, `src/components/QuestionTraining.vue`, `src/components/Dashboard.vue`: replace alerts with toast.
- Modify `src-tauri/src/models.rs`: add interview option/report action fields and question metadata.
- Modify `src-tauri/src/db.rs`: SQLite migrations and helper changes.
- Modify `src-tauri/src/lib.rs`: command payloads, start interview options, report routing data.
- Modify `src-tauri/src/llm_client.rs`: actionable report JSON parsing.

---

## Task 1: Toast System

**Files:**
- Create: `src/composables/useToast.ts`
- Create: `src/components/ui/ToastHost.vue`
- Modify: `src/components/layout/AppShell.vue`

- [ ] **Step 1: Create toast composable**

Create `src/composables/useToast.ts`:

```ts
import { readonly, ref } from "vue";

export type ToastKind = "success" | "error" | "warning" | "info";

export interface ToastItem {
  id: number;
  kind: ToastKind;
  title: string;
  message?: string;
}

const toasts = ref<ToastItem[]>([]);
let nextId = 1;

function push(kind: ToastKind, title: string, message?: string, timeout = 3200) {
  const id = nextId++;
  toasts.value = [...toasts.value, { id, kind, title, message }];
  window.setTimeout(() => dismiss(id), timeout);
  return id;
}

function dismiss(id: number) {
  toasts.value = toasts.value.filter((toast) => toast.id !== id);
}

export function useToast() {
  return {
    toasts: readonly(toasts),
    success: (title: string, message?: string) => push("success", title, message),
    error: (title: string, message?: string) => push("error", title, message, 5200),
    warning: (title: string, message?: string) => push("warning", title, message, 4400),
    info: (title: string, message?: string) => push("info", title, message),
    dismiss,
  };
}
```

- [ ] **Step 2: Create ToastHost component**

Create `src/components/ui/ToastHost.vue`:

```vue
<script setup lang="ts">
import Icon from "./Icon.vue";
import { useToast, type ToastKind } from "../../composables/useToast";

const { toasts, dismiss } = useToast();

function iconFor(kind: ToastKind) {
  if (kind === "success") return "CheckCircle2";
  if (kind === "error") return "CircleAlert";
  if (kind === "warning") return "TriangleAlert";
  return "Info";
}
</script>

<template>
  <div class="toast-host" aria-live="polite">
    <TransitionGroup name="toast">
      <div v-for="toast in toasts" :key="toast.id" :class="['toast-card', toast.kind]">
        <Icon :name="iconFor(toast.kind)" :size="16" />
        <div class="toast-copy">
          <strong>{{ toast.title }}</strong>
          <span v-if="toast.message">{{ toast.message }}</span>
        </div>
        <button class="toast-close" title="关闭" @click="dismiss(toast.id)">
          <Icon name="X" :size="14" />
        </button>
      </div>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.toast-host {
  position: fixed;
  right: 20px;
  top: 20px;
  z-index: 300;
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  width: min(360px, calc(100vw - 32px));
  pointer-events: none;
}
.toast-card {
  pointer-events: auto;
  display: grid;
  grid-template-columns: auto 1fr auto;
  gap: var(--sp-2);
  align-items: start;
  padding: var(--sp-3);
  background: var(--surface);
  border: 1px solid var(--border);
  border-left-width: 3px;
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-md);
  color: var(--text);
}
.toast-card.success { border-left-color: var(--success); }
.toast-card.error { border-left-color: var(--danger); }
.toast-card.warning { border-left-color: var(--warning); }
.toast-card.info { border-left-color: var(--accent); }
.toast-copy { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.toast-copy strong { font-size: var(--fs-13); font-weight: var(--fw-semibold); }
.toast-copy span { font-size: var(--fs-12); color: var(--text-muted); line-height: 1.45; overflow-wrap: anywhere; }
.toast-close { color: var(--text-subtle); border-radius: var(--radius-sm); padding: 2px; }
.toast-close:hover { color: var(--text); background: var(--surface-2); }
.toast-enter-active, .toast-leave-active { transition: all var(--dur-base) var(--ease); }
.toast-enter-from, .toast-leave-to { opacity: 0; transform: translateX(12px); }
</style>
```

- [ ] **Step 3: Mount ToastHost in AppShell**

Modify `src/components/layout/AppShell.vue`:

```ts
import ToastHost from "../ui/ToastHost.vue";
```

Add near the end of `.shell` template, before `</div>`:

```html
<ToastHost />
```

- [ ] **Step 4: Verify frontend build**

Run: `npm run build`

Expected: PASS.

---

## Task 2: Web Speech API Microphone Input

**Files:**
- Create: `src/composables/useSpeechRecognition.ts`
- Modify: `src/components/MockInterview.vue`

- [ ] **Step 1: Create speech composable**

Create `src/composables/useSpeechRecognition.ts`:

```ts
import { computed, onUnmounted, ref } from "vue";

type SpeechRecognitionCtor = new () => SpeechRecognition;

interface SpeechRecognitionEventLike extends Event {
  results: SpeechRecognitionResultList;
  resultIndex: number;
}

interface SpeechRecognitionErrorEventLike extends Event {
  error: string;
}

declare global {
  interface Window {
    SpeechRecognition?: SpeechRecognitionCtor;
    webkitSpeechRecognition?: SpeechRecognitionCtor;
  }
}

export function useSpeechRecognition(onText: (text: string) => void) {
  const listening = ref(false);
  const error = ref("");
  const supported = computed(() => Boolean(window.SpeechRecognition || window.webkitSpeechRecognition));
  let recognition: SpeechRecognition | null = null;

  function ensureRecognition() {
    const Ctor = window.SpeechRecognition || window.webkitSpeechRecognition;
    if (!Ctor) return null;
    if (recognition) return recognition;
    recognition = new Ctor();
    recognition.lang = "zh-CN";
    recognition.continuous = false;
    recognition.interimResults = false;
    recognition.onstart = () => {
      listening.value = true;
      error.value = "";
    };
    recognition.onend = () => {
      listening.value = false;
    };
    recognition.onerror = (event: Event) => {
      const e = event as SpeechRecognitionErrorEventLike;
      error.value = e.error || "speech-error";
      listening.value = false;
    };
    recognition.onresult = (event: Event) => {
      const e = event as SpeechRecognitionEventLike;
      const chunks: string[] = [];
      for (let i = e.resultIndex; i < e.results.length; i += 1) {
        const result = e.results.item(i);
        if (result?.isFinal && result[0]?.transcript) chunks.push(result[0].transcript.trim());
      }
      const text = chunks.filter(Boolean).join("");
      if (text) onText(text);
    };
    return recognition;
  }

  function start() {
    const rec = ensureRecognition();
    if (!rec) {
      error.value = "unsupported";
      return false;
    }
    if (listening.value) return true;
    rec.start();
    return true;
  }

  function stop() {
    if (recognition && listening.value) recognition.stop();
  }

  function toggle() {
    if (listening.value) {
      stop();
      return true;
    }
    return start();
  }

  onUnmounted(() => {
    if (recognition) recognition.abort();
  });

  return { supported, listening, error, start, stop, toggle };
}
```

- [ ] **Step 2: Wire speech into MockInterview**

Modify `src/components/MockInterview.vue` imports:

```ts
import { useToast } from "../composables/useToast";
import { useSpeechRecognition } from "../composables/useSpeechRecognition";
```

Add in script setup:

```ts
const toast = useToast();

function appendSpeechText(text: string) {
  answer.value = answer.value.trim()
    ? `${answer.value.trim()} ${text}`
    : text;
}

const speech = useSpeechRecognition(appendSpeechText);

watch(speech.error, (err) => {
  if (!err) return;
  if (err === "unsupported") {
    toast.warning("当前环境不支持语音输入", "可以继续使用键盘输入回答。");
  } else if (err === "not-allowed" || err === "service-not-allowed") {
    toast.error("麦克风权限被拒绝", "请允许应用访问麦克风后重试。");
  } else if (err === "no-speech") {
    toast.info("没有识别到语音", "可以再说一次或手动输入。");
  } else {
    toast.error("语音识别失败", err);
  }
});

function toggleSpeech() {
  const ok = speech.toggle();
  if (!ok) toast.warning("当前环境不支持语音输入", "Web Speech API 不可用。");
}
```

Add a microphone button next to the answer submit button:

```html
<button
  class="icon-btn mic-btn"
  :class="{ active: speech.listening.value }"
  :disabled="loading || !speech.supported.value"
  :title="speech.supported.value ? (speech.listening.value ? '停止语音输入' : '语音输入') : '当前环境不支持语音输入'"
  @click="toggleSpeech"
>
  <Icon :name="speech.listening.value ? 'MicOff' : 'Mic'" :size="15" />
</button>
```

Add style:

```css
.mic-btn.active {
  color: var(--danger);
  background: var(--danger-soft);
}
```

- [ ] **Step 3: Verify build**

Run: `npm run build`

Expected: PASS.

Manual verification: in Tauri dev, click microphone in mock interview. Supported environments should append recognized Chinese text into the answer box; unsupported environments should show a toast and keep manual input available.

---

## Task 3: Replace Alert Feedback in Key Pages

**Files:**
- Modify: `src/components/QuestionLibrary.vue`
- Modify: `src/components/AIGenerate.vue`
- Modify: `src/components/QuestionTraining.vue`
- Modify: `src/components/Dashboard.vue`

- [ ] **Step 1: Replace QuestionLibrary alerts**

Import:

```ts
import { useToast } from "../composables/useToast";
const toast = useToast();
```

Replace success/error alerts:

```ts
toast.success("导出完成", `已导出 ${n} 道题到 ${path}`);
toast.error("导出失败", String(e));
toast.success("已加入错题本");
toast.error("操作失败", String(e));
```

- [ ] **Step 2: Replace AIGenerate alerts**

Import toast and replace `alert(...)` calls:

```ts
toast.warning("请先配置 API Key", "在设置页填写后再生成题目。");
toast.error("生成失败", String(e));
toast.warning("请选择题目", "请至少选择一道有效题目。");
toast.error("保存失败", String(e));
toast.success("保存成功", `已保存 ${saved} 道题。`);
```

- [ ] **Step 3: Replace QuestionTraining alerts**

Import toast and replace validation/API alerts:

```ts
toast.warning("请选择考点", "请至少选择一个考点。");
toast.warning("请先作答", "填写答案后再提交。");
toast.error("评分失败", String(error));
toast.error("系统调用失败", String(error));
```

- [ ] **Step 4: Replace Dashboard delete alert**

Import toast and replace delete failure alert:

```ts
toast.error("删除失败", String(e));
```

- [ ] **Step 5: Verify alert removal and build**

Run: `rg "alert\\(" src`

Expected: no user-facing `alert(` remains in modified pages.

Run: `npm run build`

Expected: PASS.

---

## Task 4: Interview Options Data Model

**Files:**
- Create: `src/utils/interviewOptions.ts`
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add frontend option definitions**

Create `src/utils/interviewOptions.ts`:

```ts
export type InterviewDifficulty = "easy" | "medium" | "hard";
export type FollowUpIntensity = "light" | "normal" | "deep";
export type InterviewMode = "project_only" | "fundamental_only" | "full";

export interface InterviewOptions {
  targetRole: string;
  direction: string;
  difficulty: InterviewDifficulty;
  followUpIntensity: FollowUpIntensity;
  mode: InterviewMode;
}

export const defaultInterviewOptions: InterviewOptions = {
  targetRole: "后端开发",
  direction: "Rust",
  difficulty: "medium",
  followUpIntensity: "normal",
  mode: "full",
};

export const directionOptions = ["Rust", "Java", "Go", "前端", "数据库", "通用基础"];

export const difficultyLabels: Record<InterviewDifficulty, string> = {
  easy: "偏基础",
  medium: "中等",
  hard: "高压",
};

export const followUpLabels: Record<FollowUpIntensity, string> = {
  light: "轻追问",
  normal: "正常追问",
  deep: "深挖追问",
};

export const modeLabels: Record<InterviewMode, string> = {
  project_only: "只练项目",
  fundamental_only: "只练八股",
  full: "项目 + 八股",
};
```

- [ ] **Step 2: Add Rust model**

In `src-tauri/src/models.rs` add:

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InterviewOptions {
    pub target_role: String,
    pub direction: String,
    pub difficulty: String,
    pub follow_up_intensity: String,
    pub mode: String,
}
```

- [ ] **Step 3: Add DB columns and create_interview2 parameters**

In `init_db`, add idempotent migrations:

```rust
let _ = sqlx::query("ALTER TABLE mock_interviews ADD COLUMN target_role TEXT NOT NULL DEFAULT '后端开发'").execute(&pool).await;
let _ = sqlx::query("ALTER TABLE mock_interviews ADD COLUMN direction TEXT NOT NULL DEFAULT 'Rust'").execute(&pool).await;
let _ = sqlx::query("ALTER TABLE mock_interviews ADD COLUMN difficulty TEXT NOT NULL DEFAULT 'medium'").execute(&pool).await;
let _ = sqlx::query("ALTER TABLE mock_interviews ADD COLUMN follow_up_intensity TEXT NOT NULL DEFAULT 'normal'").execute(&pool).await;
let _ = sqlx::query("ALTER TABLE mock_interviews ADD COLUMN mode TEXT NOT NULL DEFAULT 'full'").execute(&pool).await;
```

Change `create_interview2` signature to accept `options: &crate::models::InterviewOptions`, insert the five fields, and update tests to pass a helper default.

- [ ] **Step 4: Update start_interview command**

In `src-tauri/src/lib.rs`, import `InterviewOptions` and change `start_interview` signature:

```rust
async fn start_interview(
    resume_id: i64,
    project_cap: i32,
    fundamental_cap: i32,
    options: InterviewOptions,
    pool: tauri::State<'_, SqlitePool>,
    config: tauri::State<'_, Mutex<AppConfig>>,
    app: tauri::AppHandle,
) -> Result<(i64, InterviewTurn), String>
```

Apply mode:

```rust
let (pc, fc) = match options.mode.as_str() {
    "project_only" => (project_cap.clamp(1, 20), 0),
    "fundamental_only" => (0, fundamental_cap.clamp(1, 20)),
    _ => (project_cap.clamp(1, 20), fundamental_cap.clamp(0, 20)),
};
```

For `fundamental_only`, create interview with phase `"fundamental"` using a new DB helper or an optional initial phase parameter.

- [ ] **Step 5: Verify backend tests**

Run: `cargo test`

Expected: PASS.

---

## Task 5: Interview Options UI and Prompt Injection

**Files:**
- Modify: `src/components/MockInterview.vue`
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add setup UI**

In `MockInterview.vue`, import defaults:

```ts
import {
  defaultInterviewOptions,
  directionOptions,
  difficultyLabels,
  followUpLabels,
  modeLabels,
  type InterviewOptions,
} from "../utils/interviewOptions";
```

Add state:

```ts
const interviewOptions = ref<InterviewOptions>({ ...defaultInterviewOptions });
```

Add fields in setup panel after round sliders:

```html
<div class="field">
  <label class="step-label"><span class="step-no">4</span>面试目标</label>
  <div class="option-grid">
    <input v-model="interviewOptions.targetRole" class="fr-input" placeholder="目标岗位，例如 后端开发" />
    <select v-model="interviewOptions.direction" class="fr-input">
      <option v-for="d in directionOptions" :key="d" :value="d">{{ d }}</option>
    </select>
    <select v-model="interviewOptions.difficulty" class="fr-input">
      <option v-for="(label, key) in difficultyLabels" :key="key" :value="key">{{ label }}</option>
    </select>
    <select v-model="interviewOptions.followUpIntensity" class="fr-input">
      <option v-for="(label, key) in followUpLabels" :key="key" :value="key">{{ label }}</option>
    </select>
    <select v-model="interviewOptions.mode" class="fr-input">
      <option v-for="(label, key) in modeLabels" :key="key" :value="key">{{ label }}</option>
    </select>
  </div>
</div>
```

- [ ] **Step 2: Pass options to backend**

In `startInterview`, pass:

```ts
options: {
  targetRole: interviewOptions.value.targetRole,
  direction: interviewOptions.value.direction,
  difficulty: interviewOptions.value.difficulty,
  followUpIntensity: interviewOptions.value.followUpIntensity,
  mode: interviewOptions.value.mode,
}
```

- [ ] **Step 3: Inject options into prompt**

In `build_interviewer_system`, add `options` context loaded from DB:

```rust
let difficulty_cn = match options.difficulty.as_str() {
    "easy" => "偏基础",
    "hard" => "高压深入",
    _ => "中等",
};
let follow_cn = match options.follow_up_intensity.as_str() {
    "light" => "轻追问",
    "deep" => "深挖追问",
    _ => "正常追问",
};
```

Include in prompt:

```rust
"目标岗位：{target_role}；方向：{direction}；难度：{difficulty_cn}；追问强度：{follow_cn}。"
```

- [ ] **Step 4: Verify build and tests**

Run: `cargo test`

Run: `npm run build`

Expected: both PASS.

---

## Task 6: Actionable Interview Report

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/llm_client.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/components/MockInterview.vue`
- Modify: `src/components/InterviewHistory.vue`

- [ ] **Step 1: Add report fields**

In `InterviewReport2` add:

```rust
pub weak_points: Vec<String>,
pub recommended_tags: Vec<String>,
pub action_items: Vec<String>,
```

Add columns:

```rust
let _ = sqlx::query("ALTER TABLE mock_interviews ADD COLUMN weak_points TEXT NOT NULL DEFAULT '[]'").execute(&pool).await;
let _ = sqlx::query("ALTER TABLE mock_interviews ADD COLUMN recommended_tags TEXT NOT NULL DEFAULT '[]'").execute(&pool).await;
let _ = sqlx::query("ALTER TABLE mock_interviews ADD COLUMN action_items TEXT NOT NULL DEFAULT '[]'").execute(&pool).await;
```

- [ ] **Step 2: Extend LLM report parser**

Change `evaluate_interview` return type to:

```rust
Result<(crate::models::DimensionScores, String, Vec<String>, Vec<String>, Vec<String>), String>
```

Prompt JSON:

```rust
r#"{"project_depth":85,"fundamental_solidity":70,"communication":80,"summary":"...","weak_points":["..."],"recommended_tags":["数据库"],"action_items":["..."]}"#
```

Parse arrays with:

```rust
fn read_string_array(v: &Value, key: &str) -> Vec<String> {
    v[key].as_array()
        .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.trim().to_string())).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default()
}
```

- [ ] **Step 3: Store and read action fields**

Update `finish_interview2` to accept and store the three JSON arrays.

Update `get_interview_meta` to return `(average_score, dimension_scores, summary, weak_points, recommended_tags, action_items)`.

- [ ] **Step 4: Show report sections in frontend**

In both report views add:

```html
<div class="fr-card action-card">
  <h3>下一步建议</h3>
  <div class="action-section">
    <span>薄弱点</span>
    <ul><li v-for="item in report.weak_points" :key="item">{{ item }}</li></ul>
  </div>
  <div class="action-section">
    <span>建议练习标签</span>
    <div class="chips"><span v-for="tag in report.recommended_tags" :key="tag" class="fr-chip fr-chip-accent">{{ tag }}</span></div>
  </div>
  <div class="action-section">
    <span>行动项</span>
    <ul><li v-for="item in report.action_items" :key="item">{{ item }}</li></ul>
  </div>
</div>
```

- [ ] **Step 5: Verify tests/build**

Run: `cargo test`

Run: `npm run build`

Expected: both PASS.

---

## Task 7: Question Quality Controls

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/components/QuestionLibrary.vue`
- Modify: `src/components/ui/QuestionModal.vue`
- Modify: `src/components/AIGenerate.vue`

- [ ] **Step 1: Add question metadata fields**

In `Question` add:

```rust
pub source: String,
pub review_status: String,
pub quality_note: String,
```

In `init_db` add:

```rust
let _ = sqlx::query("ALTER TABLE questions ADD COLUMN source TEXT NOT NULL DEFAULT 'manual'").execute(&pool).await;
let _ = sqlx::query("ALTER TABLE questions ADD COLUMN review_status TEXT NOT NULL DEFAULT 'approved'").execute(&pool).await;
let _ = sqlx::query("ALTER TABLE questions ADD COLUMN quality_note TEXT NOT NULL DEFAULT ''").execute(&pool).await;
```

- [ ] **Step 2: Extend create/update functions**

Add parameters to `create_question` and `update_question`:

```rust
source: &str,
review_status: &str,
quality_note: &str,
```

Manual create defaults:

```rust
"manual", "approved", ""
```

AI save defaults:

```rust
"ai_generated", "needs_review", ""
```

Import defaults:

```rust
"imported", "needs_review", ""
```

- [ ] **Step 3: Update QuestionModal form**

Add fields:

```ts
source: "manual",
review_status: "approved",
quality_note: "",
```

Add UI:

```html
<div class="row two">
  <div>
    <label>来源</label>
    <select v-model="form.source" class="fr-input" :disabled="isView">
      <option value="manual">手动</option>
      <option value="ai_generated">AI 生成</option>
      <option value="imported">导入</option>
    </select>
  </div>
  <div>
    <label>审核状态</label>
    <select v-model="form.review_status" class="fr-input" :disabled="isView">
      <option value="approved">已审核</option>
      <option value="needs_review">待复核</option>
      <option value="draft">草稿</option>
    </select>
  </div>
</div>
<div class="row">
  <label>质量备注</label>
  <textarea v-model="form.quality_note" class="fr-input" rows="2" :disabled="isView"></textarea>
</div>
```

- [ ] **Step 4: Show metadata in QuestionLibrary**

Add badges:

```html
<span class="fr-chip">{{ q.source === 'ai_generated' ? 'AI' : q.source === 'imported' ? '导入' : '手动' }}</span>
<span :class="['fr-chip', q.review_status === 'approved' ? 'fr-chip-accent' : '']">
  {{ q.review_status === 'approved' ? '已审核' : q.review_status === 'needs_review' ? '待复核' : '草稿' }}
</span>
```

- [ ] **Step 5: Verify backend and frontend**

Run: `cargo test`

Run: `npm run build`

Expected: both PASS.

---

## Task 8: Shared Radar and Cleanup

**Files:**
- Create: `src/components/ui/InterviewRadar.vue`
- Modify: `src/components/MockInterview.vue`
- Modify: `src/components/InterviewHistory.vue`

- [ ] **Step 1: Extract shared radar component**

Create `InterviewRadar.vue` with props:

```ts
const props = defineProps<{
  scores: { project_depth: number; fundamental_solidity: number; communication: number };
}>();
```

Move existing ECharts setup and render logic into the component.

- [ ] **Step 2: Replace inline radar code**

In `MockInterview.vue` and `InterviewHistory.vue`, remove ECharts imports/state/render functions and use:

```html
<InterviewRadar :scores="report.dimension_scores" />
```

- [ ] **Step 3: Verify build**

Run: `npm run build`

Expected: PASS and no duplicate radar render code in both pages.

---

## Task 9: Final Verification

**Files:**
- All modified files.

- [ ] **Step 1: Search for remaining alerts**

Run: `rg "alert\\(" src`

Expected: no unexpected user-facing alerts.

- [ ] **Step 2: Search for old mock interview commands**

Run: `rg "start_mock_interview|submit_mock_answer|submit_mock_follow_up|record_skipped_question|finish_mock_interview" src src-tauri/src`

Expected: no matches.

- [ ] **Step 3: Run backend tests**

Run: `cargo test`

Expected: PASS.

- [ ] **Step 4: Run frontend build**

Run: `npm run build`

Expected: PASS. Vite chunk-size warning is acceptable unless new errors appear.

- [ ] **Step 5: Manual GUI checklist**

Run: `npm run tauri dev`

Verify:

1. Toast notifications appear for export/add wrong/delete/generate errors.
2. Mock interview answer box has microphone button and graceful unsupported state.
3. Interview setup options are sent and affect prompt behavior.
4. Report shows weak points, recommended tags, and action items.
5. AI-generated/imported questions show `待复核`; manual questions show `已审核`.

---

## Self-Review

- Spec coverage: speech input, toast, interview options, actionable reports, question quality, and cleanup each have a task.
- Placeholder scan: no `TBD` or open-ended implementation placeholders remain.
- Type consistency: frontend camelCase payloads map to Tauri snake_case parameters; Rust `InterviewOptions` uses snake_case fields and Vue sends `targetRole`, `followUpIntensity` via Tauri's casing conversion.
