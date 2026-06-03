# 阶段二·B 模拟面试前端重写 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `MockInterview.vue` 从「随机抽题一问一答」重写为「上传简历 → 对话式流式面试（项目/八股两环节）→ 多维雷达图复盘」，对接阶段二·A 已落地的后端命令与 `interview-token` 流式事件。

**Architecture:** 前端用 webview 内 `<input type=file>` + `pdfjs-dist` 抽 PDF 文本（不加 Tauri fs 插件）。新增 `useInterviewStream` composable 监听 `interview-token` 事件累积流式文本。`MockInterview.vue` 三阶段 `resume / interview / report`，复用现有设计 token 与视频面试风样式。报告用 ECharts 雷达图（模块化引入 `RadarChart`）。最后清理已被取代的旧后端命令。

**Tech Stack:** Vue 3 `<script setup>` + TypeScript + Vite + pdfjs-dist + ECharts + Tauri 2 event API。

**后端契约（阶段二·A 已实现，前端按此对接）：**
- `parse_resume({ rawText })` → `ResumeRecord { id, candidate, projects:[{name,role,summary,highlights}], tech_stack:[] }`
- `start_interview({ resumeId, projectCap, fundamentalCap })` → `[interviewId, { message, phase, finished }]`（元组，JSON 数组）
- `interview_respond({ interviewId, answer })` → `{ message, phase, finished }`
- `finish_interview({ interviewId })` → `{ interview_id, average_score, dimension_scores:{project_depth,fundamental_solidity,communication}, summary, messages:[{role,phase,content,seq}] }`
- 流式事件 `interview-token`，负载 `{ interviewId, chunk }`

**约束：代码尽量精简**——复用现有 token/样式/Icon、`inject("apiKey")` 模式；拆出 composable 让组件聚焦 UI；不引入多余依赖。

---

## 文件结构

- 修改 `package.json`：新增 `pdfjs-dist` 依赖。
- 新建 `src/utils/pdfText.ts`：PDF File → 纯文本。
- 新建 `src/composables/useInterviewStream.ts`：监听 `interview-token`、累积流式文本。
- 重写 `src/components/MockInterview.vue`：三阶段（resume/interview/report）。
- 修改 `src-tauri/src/lib.rs` / `llm_client.rs` / `models.rs`：清理被取代的旧模拟面试命令（最后一步）。

---

## Task 1: 引入 pdfjs-dist 与 PDF 文本抽取工具

**Files:**
- Modify: `package.json`
- Create: `src/utils/pdfText.ts`

- [ ] **Step 1: 安装依赖**

Run: `cd "e:/2026_Junior_S2/Rust/Project/ForgeRust" && npm install pdfjs-dist@^4`
Expected: `package.json` dependencies 出现 `pdfjs-dist`，安装成功。

- [ ] **Step 2: 创建 PDF 抽取工具**

创建 `src/utils/pdfText.ts`：

```ts
import * as pdfjsLib from "pdfjs-dist";
// Vite 专用：?url 拿到 worker 资源地址
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";

pdfjsLib.GlobalWorkerOptions.workerSrc = workerUrl;

// 从用户选择的 PDF File 抽取纯文本
export async function extractPdfText(file: File): Promise<string> {
  const buf = await file.arrayBuffer();
  const pdf = await pdfjsLib.getDocument({ data: new Uint8Array(buf) }).promise;
  let text = "";
  for (let i = 1; i <= pdf.numPages; i++) {
    const page = await pdf.getPage(i);
    const content = await page.getTextContent();
    text += content.items.map((it: any) => ("str" in it ? it.str : "")).join(" ") + "\n";
  }
  return text.trim();
}
```

- [ ] **Step 3: 构建验证**

Run: `cd "e:/2026_Junior_S2/Rust/Project/ForgeRust" && npm run build`
Expected: 构建无类型/打包错误（pdfjs worker 通过 `?url` 正常解析）。

- [ ] **Step 4: Commit**

```bash
git add package.json package-lock.json src/utils/pdfText.ts
git commit -m "feat: 引入 pdfjs-dist 并新增 PDF 文本抽取工具"
```

---

## Task 2: useInterviewStream composable

**Files:**
- Create: `src/composables/useInterviewStream.ts`

- [ ] **Step 1: 创建 composable**

仿照现有 `useImportProgress.ts` 的 `listen` 模式。创建 `src/composables/useInterviewStream.ts`：

```ts
import { ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

interface TokenEvent {
  interviewId: number;
  chunk: string;
}

// 监听后端 interview-token 事件，把流式文本累积到 streamingText。
// 单窗口单场面试，无需按 interviewId 过滤；组件在每轮开始前 resetStream()。
export function useInterviewStream() {
  const streamingText = ref("");
  let unlisten: UnlistenFn | null = null;

  async function ensureListener() {
    if (unlisten) return;
    unlisten = await listen<TokenEvent>("interview-token", (e) => {
      streamingText.value += e.payload.chunk;
    });
  }

  function resetStream() {
    streamingText.value = "";
  }

  function stop() {
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
  }

  return { streamingText, ensureListener, resetStream, stop };
}
```

- [ ] **Step 2: 构建验证**

Run: `cd "e:/2026_Junior_S2/Rust/Project/ForgeRust" && npm run build`
Expected: 无类型错误。

- [ ] **Step 3: Commit**

```bash
git add src/composables/useInterviewStream.ts
git commit -m "feat: 新增 useInterviewStream 流式事件 composable"
```

---

## Task 3: 重写 MockInterview.vue（resume / interview / report 三阶段）

**Files:**
- Modify: `src/components/MockInterview.vue`（整文件重写；report 阶段先用文本展示三维分，雷达图在 Task 4 加入）

- [ ] **Step 1: 整文件替换**

用以下完整内容覆盖 `src/components/MockInterview.vue`：

```vue
<script setup lang="ts">
import { computed, inject, nextTick, onUnmounted, ref, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Icon from "./ui/Icon.vue";
import { extractPdfText } from "../utils/pdfText";
import { useInterviewStream } from "../composables/useInterviewStream";

interface ResumeProject { name: string; role: string; summary: string; highlights: string[]; }
interface ResumeRecord { id: number; candidate: string; projects: ResumeProject[]; tech_stack: string[]; }
interface InterviewTurn { message: string; phase: string; finished: boolean; }
interface ChatMessage { role: "interviewer" | "candidate"; phase: string; text: string; }
interface DimensionScores { project_depth: number; fundamental_solidity: number; communication: number; }
interface InterviewReport {
  interview_id: number;
  average_score: number;
  dimension_scores: DimensionScores;
  summary: string;
  messages: { role: string; phase: string; content: string; seq: number }[];
}

const apiKey = inject<Ref<string>>("apiKey", ref(""));
const hasApiKey = computed(() => !!apiKey.value.trim());

const { streamingText, ensureListener, resetStream, stop } = useInterviewStream();

const stage = ref<"resume" | "interview" | "report">("resume");
const loading = ref(false);
const errorMsg = ref("");

// resume 阶段
const parsing = ref(false);
const resume = ref<ResumeRecord | null>(null);
const projectCap = ref(5);
const fundamentalCap = ref(5);
const capPresets = [3, 5, 8];

// interview 阶段
const interviewId = ref<number | null>(null);
const currentPhase = ref<"project" | "fundamental">("project");
const finished = ref(false);
const history = ref<ChatMessage[]>([]);
const currentPrompt = ref("");      // 面试官当前已定稿的提问
const answer = ref("");
const transcriptRef = ref<HTMLElement | null>(null);
const elapsed = ref(0);
let clockTimer: ReturnType<typeof setInterval> | null = null;
const confirmingExit = ref(false);

// report 阶段
const report = ref<InterviewReport | null>(null);

const phaseLabel = computed(() => (currentPhase.value === "project" ? "项目环节" : "八股环节"));
const elapsedText = computed(() => {
  const m = Math.floor(elapsed.value / 60).toString().padStart(2, "0");
  const s = (elapsed.value % 60).toString().padStart(2, "0");
  return `${m}:${s}`;
});
// 面试官气泡：流式中显示 streamingText，定稿后显示 currentPrompt
const interviewerText = computed(() => (loading.value ? streamingText.value : currentPrompt.value));

function startClock() {
  stopClock();
  elapsed.value = 0;
  clockTimer = setInterval(() => { elapsed.value += 1; }, 1000);
}
function stopClock() {
  if (clockTimer) { clearInterval(clockTimer); clockTimer = null; }
}
function scrollTranscript() {
  nextTick(() => {
    const el = transcriptRef.value;
    if (el) el.scrollTop = el.scrollHeight;
  });
}

onUnmounted(() => { stopClock(); stop(); });

// ── resume 阶段 ──
async function onPickPdf(e: Event) {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = ""; // 允许重选同一文件
  if (!file) return;
  if (!hasApiKey.value) { errorMsg.value = "请先在设置页配置 API Key。"; return; }
  parsing.value = true;
  errorMsg.value = "";
  try {
    const text = await extractPdfText(file);
    if (!text) { errorMsg.value = "未能从 PDF 抽取到文本（可能是扫描件）。"; return; }
    resume.value = await invoke<ResumeRecord>("parse_resume", { rawText: text });
  } catch (err) {
    errorMsg.value = String(err);
  } finally {
    parsing.value = false;
  }
}
function removeTech(i: number) { resume.value?.tech_stack.splice(i, 1); }
function removeProject(i: number) { resume.value?.projects.splice(i, 1); }

async function startInterview() {
  if (!resume.value) return;
  loading.value = true;
  errorMsg.value = "";
  history.value = [];
  resetStream();
  await ensureListener();
  try {
    const [id, turn] = await invoke<[number, InterviewTurn]>("start_interview", {
      resumeId: resume.value.id,
      projectCap: projectCap.value,
      fundamentalCap: fundamentalCap.value,
    });
    interviewId.value = id;
    applyTurn(turn);
    stage.value = "interview";
    startClock();
  } catch (err) {
    errorMsg.value = String(err);
  } finally {
    loading.value = false;
    resetStream();
  }
}

// ── interview 阶段 ──
function applyTurn(turn: InterviewTurn) {
  currentPrompt.value = turn.message;
  currentPhase.value = (turn.phase as "project" | "fundamental") ?? "project";
  finished.value = turn.finished;
  scrollTranscript();
}

async function submitAnswer() {
  if (!interviewId.value || !answer.value.trim() || loading.value) return;
  const myAnswer = answer.value.trim();
  // 先把上一轮面试官提问与本次回答沉入历史
  history.value = [
    ...history.value,
    { role: "interviewer", phase: currentPhase.value, text: currentPrompt.value },
    { role: "candidate", phase: currentPhase.value, text: myAnswer },
  ];
  answer.value = "";
  currentPrompt.value = "";
  loading.value = true;
  errorMsg.value = "";
  resetStream();
  scrollTranscript();
  try {
    const turn = await invoke<InterviewTurn>("interview_respond", {
      interviewId: interviewId.value,
      answer: myAnswer,
    });
    applyTurn(turn);
    if (turn.finished) await finishInterview();
  } catch (err) {
    errorMsg.value = String(err);
  } finally {
    loading.value = false;
    resetStream();
  }
}

async function finishInterview() {
  if (!interviewId.value) return;
  stopClock();
  // 把最后一轮面试官收尾语沉入历史
  if (currentPrompt.value) {
    history.value = [...history.value, { role: "interviewer", phase: currentPhase.value, text: currentPrompt.value }];
    currentPrompt.value = "";
  }
  try {
    report.value = await invoke<InterviewReport>("finish_interview", { interviewId: interviewId.value });
    stage.value = "report";
  } catch (err) {
    errorMsg.value = String(err);
  }
}

function requestExit() { confirmingExit.value = true; }
function cancelExit() { confirmingExit.value = false; }
function confirmExit() { reset(); }

function reset() {
  stopClock();
  stage.value = "resume";
  resume.value = null;
  interviewId.value = null;
  history.value = [];
  currentPrompt.value = "";
  answer.value = "";
  finished.value = false;
  report.value = null;
  confirmingExit.value = false;
  errorMsg.value = "";
  resetStream();
}
</script>

<template>
  <div class="fr-page mock-page">
    <header class="mock-head">
      <div>
        <h1 class="fr-page-title">模拟面试</h1>
        <p class="fr-page-subtitle">上传简历，AI 面试官分「项目」「八股」两环节与你多轮对话，结束生成多维复盘。</p>
      </div>
      <span v-if="stage === 'interview'" class="mode-chip">{{ phaseLabel }}</span>
    </header>

    <!-- ── resume 阶段 ── -->
    <section v-if="stage === 'resume'" class="setup-panel">
      <div class="fr-card setup-card">
        <div class="field">
          <label class="step-label"><span class="step-no">1</span>上传简历（PDF）</label>
          <label class="upload-zone">
            <input type="file" accept="application/pdf,.pdf" @change="onPickPdf" hidden />
            <Icon name="Upload" :size="18" />
            <span>{{ parsing ? "正在解析简历…" : (resume ? "重新选择 PDF" : "点击选择 PDF 文件") }}</span>
          </label>
          <p v-if="!hasApiKey" class="notice"><Icon name="AlertTriangle" :size="16" /><span>模拟面试需要先在设置页配置 API Key。</span></p>
        </div>

        <template v-if="resume">
          <div class="field">
            <label class="step-label"><span class="step-no">2</span>识别结果（可删除误识别项）</label>
            <p class="resume-name">候选人：{{ resume.candidate || "（未识别）" }}</p>
            <div class="chips">
              <span v-for="(t, i) in resume.tech_stack" :key="t + i" class="chip-removable">
                {{ t }}<button @click="removeTech(i)"><Icon name="X" :size="12" /></button>
              </span>
              <span v-if="!resume.tech_stack.length" class="empty-hint">未识别到技术栈</span>
            </div>
            <ul class="project-list">
              <li v-for="(p, i) in resume.projects" :key="p.name + i">
                <div class="proj-head"><strong>{{ p.name }}</strong><button class="icon-btn danger" @click="removeProject(i)"><Icon name="X" :size="14" /></button></div>
                <p class="proj-sum">{{ p.summary }}</p>
              </li>
              <li v-if="!resume.projects.length" class="empty-hint">未识别到项目</li>
            </ul>
          </div>

          <div class="field">
            <label class="step-label"><span class="step-no">3</span>环节轮数上限</label>
            <div class="cap-row">
              <span>项目</span>
              <button v-for="n in capPresets" :key="'p'+n" :class="['choice-chip', { active: projectCap === n }]" @click="projectCap = n">{{ n }}</button>
              <span class="cap-sep">八股</span>
              <button v-for="n in capPresets" :key="'f'+n" :class="['choice-chip', { active: fundamentalCap === n }]" @click="fundamentalCap = n">{{ n }}</button>
            </div>
          </div>

          <div class="setup-footer">
            <span class="setup-summary">项目 {{ projectCap }} 轮 · 八股 {{ fundamentalCap }} 轮</span>
            <button class="fr-btn fr-btn-primary" :disabled="loading" @click="startInterview">
              <Icon name="MessageSquare" :size="14" /><span>{{ loading ? "准备中…" : "开始面试" }}</span>
            </button>
          </div>
        </template>

        <p v-if="errorMsg" class="error-msg">{{ errorMsg }}</p>
      </div>
    </section>

    <!-- ── interview 阶段 ── -->
    <section v-else-if="stage === 'interview'" class="interview-panel">
      <div class="interview-bar">
        <div v-if="confirmingExit" class="exit-confirm">
          <span>确定退出？本场进度不会保存。</span>
          <button class="fr-btn fr-btn-ghost" @click="cancelExit">取消</button>
          <button class="fr-btn fr-btn-danger" @click="confirmExit">确定退出</button>
        </div>
        <button v-else class="fr-btn fr-btn-ghost" :disabled="loading" @click="requestExit"><Icon name="X" :size="14" /><span>退出面试</span></button>
      </div>

      <div class="video-bar">
        <div class="video-tile interviewer-tile">
          <span class="rec-dot"><span class="dot"></span>录制中</span>
          <span class="clock fr-mono">{{ elapsedText }}</span>
          <div class="avatar">AI</div>
          <span class="tile-label">AI 面试官 · {{ phaseLabel }}</span>
        </div>
        <div class="video-tile candidate-tile"><Icon name="VideoOff" :size="18" /><span class="cam-hint">摄像头未开启</span><span class="tile-label">你</span></div>
      </div>

      <div v-if="history.length" ref="transcriptRef" class="transcript">
        <div v-for="(msg, i) in history" :key="i" :class="['bubble-row', msg.role]"><div class="bubble">{{ msg.text }}</div></div>
      </div>

      <div class="prompt-box">
        <span class="prompt-label">面试官</span>
        <p class="prompt-text">{{ interviewerText }}<span v-if="loading" class="caret">▌</span></p>
        <span v-if="loading && !streamingText" class="thinking"><Icon name="Loader" :size="13" /> 面试官思考中…</span>
      </div>

      <div class="answer-card">
        <label>你的回答</label>
        <textarea v-model="answer" class="fr-input answer-input" :rows="6" :disabled="loading" placeholder="像真实面试一样组织回答：先结论，再展开关键点。"></textarea>
        <div class="answer-actions">
          <button class="fr-btn fr-btn-primary" :disabled="loading || !answer.trim()" @click="submitAnswer"><Icon name="Send" :size="14" /><span>{{ loading ? "提交中…" : "回答" }}</span></button>
        </div>
      </div>
      <p v-if="errorMsg" class="error-msg">{{ errorMsg }}</p>
    </section>

    <!-- ── report 阶段 ── -->
    <section v-else-if="report" class="report-panel">
      <div class="fr-card report-hero">
        <div>
          <span class="report-label">面试复盘</span>
          <h2>{{ report.average_score.toFixed(1) }}</h2>
          <p>{{ report.summary }}</p>
        </div>
        <button class="fr-btn fr-btn-ghost" @click="reset"><Icon name="RotateCcw" :size="14" /><span>再来一场</span></button>
      </div>

      <div class="fr-card dim-card">
        <h3>能力维度</h3>
        <ul class="dim-list">
          <li><span>项目深度</span><b>{{ report.dimension_scores.project_depth }}</b></li>
          <li><span>八股扎实度</span><b>{{ report.dimension_scores.fundamental_solidity }}</b></li>
          <li><span>表达逻辑</span><b>{{ report.dimension_scores.communication }}</b></li>
        </ul>
      </div>

      <div class="transcript-replay">
        <div v-for="(m, i) in report.messages" :key="i" :class="['bubble-row', m.role === 'interviewer' ? 'interviewer' : 'candidate']">
          <div class="bubble">{{ m.content }}</div>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.mock-page { max-width: var(--content-max); margin: 0 auto; }
.mock-head { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--sp-4); }
.mode-chip { font-size: var(--fs-12); color: var(--accent); background: var(--accent-soft); border-radius: 999px; padding: 4px 10px; }
.setup-panel, .interview-panel, .report-panel { display: flex; flex-direction: column; gap: var(--sp-4); }
.setup-card { display: flex; flex-direction: column; gap: var(--sp-6); }
.field { display: flex; flex-direction: column; gap: var(--sp-2); }
.step-label { display: inline-flex; align-items: center; gap: 6px; font-size: var(--fs-12); color: var(--text-muted); font-weight: var(--fw-medium); }
.step-no { display: inline-flex; align-items: center; justify-content: center; width: 18px; height: 18px; border-radius: 50%; background: var(--accent-soft); color: var(--accent); font-size: 11px; font-weight: var(--fw-semibold); }

.upload-zone { display: flex; align-items: center; justify-content: center; gap: var(--sp-2); padding: var(--sp-6); border: 1px dashed var(--border-strong); border-radius: var(--radius-lg); color: var(--text-muted); cursor: pointer; transition: all var(--dur-fast) var(--ease); }
.upload-zone:hover { border-color: var(--accent); color: var(--accent); background: var(--accent-soft); }

.resume-name { font-size: var(--fs-13); color: var(--text); }
.chips { display: flex; flex-wrap: wrap; gap: 6px; }
.chip-removable { display: inline-flex; align-items: center; gap: 4px; padding: 4px 6px 4px 10px; border-radius: 999px; font-size: var(--fs-12); background: var(--accent-soft); color: var(--accent); }
.chip-removable button { display: inline-flex; }
.empty-hint { font-size: var(--fs-12); color: var(--text-subtle); }
.project-list { display: flex; flex-direction: column; gap: var(--sp-2); }
.project-list li { padding: var(--sp-2) var(--sp-3); border: 1px solid var(--border); border-radius: var(--radius-md); }
.proj-head { display: flex; justify-content: space-between; align-items: center; }
.proj-sum { font-size: var(--fs-12); color: var(--text-muted); margin-top: 2px; }

.cap-row { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
.cap-row > span { font-size: var(--fs-13); color: var(--text-muted); }
.cap-sep { margin-left: var(--sp-3); }
.choice-chip { min-width: 44px; padding: 6px 12px; border-radius: var(--radius-md); font-size: var(--fs-13); color: var(--text-muted); background: var(--surface); border: 1px solid var(--border); cursor: pointer; }
.choice-chip.active { color: var(--text-on-accent); background: var(--accent); border-color: transparent; }

.setup-footer { display: flex; align-items: center; justify-content: space-between; gap: var(--sp-4); padding-top: var(--sp-4); border-top: 1px solid var(--border); }
.setup-summary { font-size: var(--fs-13); color: var(--text-muted); }
.notice, .error-msg { display: flex; align-items: center; gap: var(--sp-2); font-size: var(--fs-13); }
.notice { color: var(--warning); }
.error-msg { color: var(--danger); }

.interview-bar { display: flex; justify-content: flex-end; }
.exit-confirm { display: flex; align-items: center; gap: var(--sp-2); font-size: var(--fs-13); color: var(--text-muted); }
.video-bar { display: flex; gap: var(--sp-3); }
.video-tile { position: relative; height: 120px; border-radius: var(--radius-lg); border: 1px solid var(--border); background: var(--surface); box-shadow: var(--shadow-sm); display: flex; align-items: center; justify-content: center; }
.interviewer-tile { flex: 1; }
.candidate-tile { width: 160px; flex-shrink: 0; flex-direction: column; gap: 4px; background: var(--surface-2); color: var(--text-subtle); }
.avatar { width: 46px; height: 46px; border-radius: 50%; background: var(--accent); color: var(--text-on-accent); display: flex; align-items: center; justify-content: center; font-size: var(--fs-16); font-weight: var(--fw-semibold); }
.rec-dot { position: absolute; top: 9px; left: 11px; display: inline-flex; align-items: center; gap: 5px; font-size: var(--fs-12); color: var(--danger); }
.rec-dot .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--danger); animation: rec-pulse 1.4s ease-in-out infinite; }
@keyframes rec-pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.3; } }
.clock { position: absolute; top: 9px; right: 11px; font-size: var(--fs-12); color: var(--text-subtle); }
.tile-label { position: absolute; bottom: 9px; left: 11px; font-size: var(--fs-12); color: var(--text-muted); }
.cam-hint { font-size: var(--fs-12); }

.transcript { max-height: 240px; overflow-y: auto; display: flex; flex-direction: column; gap: var(--sp-2); padding: var(--sp-1); }
.transcript-replay { display: flex; flex-direction: column; gap: var(--sp-2); }
.bubble-row { display: flex; }
.bubble-row.interviewer { justify-content: flex-start; }
.bubble-row.candidate { justify-content: flex-end; }
.bubble { max-width: 78%; padding: 8px 12px; font-size: var(--fs-13); line-height: 1.6; border-radius: var(--radius-lg); }
.bubble-row.interviewer .bubble { background: var(--accent-soft); color: var(--text); border-bottom-left-radius: var(--radius-sm); }
.bubble-row.candidate .bubble { background: var(--surface-2); color: var(--text); border: 1px solid var(--border); border-bottom-right-radius: var(--radius-sm); }

.prompt-box { background: var(--accent-soft); border-left: 3px solid var(--accent); border-radius: var(--radius-md); padding: var(--sp-3) var(--sp-4); display: flex; flex-direction: column; gap: var(--sp-1); }
.prompt-label { font-size: var(--fs-12); font-weight: var(--fw-medium); color: var(--accent); }
.prompt-text { font-size: var(--fs-16); line-height: 1.7; color: var(--text); }
.caret { opacity: 0.45; animation: caret-blink 1s step-end infinite; }
@keyframes caret-blink { 50% { opacity: 0; } }
.thinking { display: inline-flex; align-items: center; gap: 5px; font-size: var(--fs-12); color: var(--text-muted); }

.answer-card { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: var(--sp-6); box-shadow: var(--shadow-sm); display: flex; flex-direction: column; gap: var(--sp-3); }
.answer-card label { font-size: var(--fs-12); color: var(--text-muted); font-weight: var(--fw-medium); }
.answer-input { resize: vertical; line-height: 1.6; }
.answer-actions { display: flex; justify-content: flex-end; gap: var(--sp-2); }

.report-hero { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--sp-4); }
.report-label { font-size: var(--fs-12); color: var(--text-muted); }
.report-hero h2 { font-size: var(--fs-28); color: var(--accent); margin: var(--sp-1) 0; }
.report-hero p { color: var(--text-muted); line-height: 1.6; }
.dim-card { display: flex; flex-direction: column; gap: var(--sp-3); }
.dim-card h3 { font-size: var(--fs-14); font-weight: var(--fw-semibold); }
.dim-list { display: flex; gap: var(--sp-4); }
.dim-list li { display: flex; flex-direction: column; gap: 2px; }
.dim-list li span { font-size: var(--fs-12); color: var(--text-muted); }
.dim-list li b { font-size: var(--fs-20); color: var(--accent); font-family: var(--font-mono); }

.icon-btn { width: 26px; height: 26px; border-radius: var(--radius-sm); color: var(--text-subtle); display: inline-flex; align-items: center; justify-content: center; }
.icon-btn.danger:hover { color: var(--danger); background: var(--danger-soft); }

@media (max-width: 760px) {
  .report-hero, .video-bar { flex-direction: column; }
  .candidate-tile { width: 100%; height: 80px; flex-direction: row; }
}
</style>
```

- [ ] **Step 2: 构建验证**

Run: `cd "e:/2026_Junior_S2/Rust/Project/ForgeRust" && npm run build`
Expected: vue-tsc + Vite 构建无错误。

- [ ] **Step 3: Commit**

```bash
git add src/components/MockInterview.vue
git commit -m "feat: 模拟面试前端重写为简历驱动的对话式流式面试"
```

---

## Task 4: 报告阶段加入 ECharts 雷达图

**Files:**
- Modify: `src/components/MockInterview.vue`

- [ ] **Step 1: 引入 echarts 与雷达图模块**

在 `<script setup>` 顶部 import 区（`useInterviewStream` import 之后）加入：

```ts
import * as echarts from "echarts/core";
import { RadarChart } from "echarts/charts";
import { TooltipComponent } from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
echarts.use([RadarChart, TooltipComponent, CanvasRenderer]);
```

并把第一行原有的 `import { computed, inject, nextTick, onUnmounted, ref, type Ref } from "vue";` 改为补上 `shallowRef, watch`：
`import { computed, inject, nextTick, onUnmounted, ref, shallowRef, watch, type Ref } from "vue";`

- [ ] **Step 2: 新增雷达图渲染逻辑**

在 `report` ref 声明之后加入：

```ts
const radarEl = ref<HTMLElement | null>(null);
const radarChart = shallowRef<echarts.ECharts | null>(null);

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

// 进入 report 阶段且 DOM 就绪后渲染
watch([stage, report], () => {
  if (stage.value === "report" && report.value) {
    nextTick(renderRadar);
  }
});
```

并把 `onUnmounted(() => { stopClock(); stop(); });` 改为：

```ts
onUnmounted(() => { stopClock(); stop(); radarChart.value?.dispose(); });
```

- [ ] **Step 3: 模板中用雷达图替换文本维度列表**

把 Task 3 的 `.dim-card` 块替换为：

```html
<div class="fr-card dim-card">
  <h3>能力维度</h3>
  <div ref="radarEl" class="radar"></div>
</div>
```

并删除 `<style scoped>` 中已不再使用的 `.dim-list` 三条规则，新增：

```css
.radar { width: 100%; height: 280px; }
```

- [ ] **Step 4: 构建验证**

Run: `cd "e:/2026_Junior_S2/Rust/Project/ForgeRust" && npm run build`
Expected: 构建无错误。

- [ ] **Step 5: Commit**

```bash
git add src/components/MockInterview.vue
git commit -m "feat: 面试报告用 ECharts 雷达图展示三维能力"
```

---

## Task 5: 清理被取代的旧模拟面试后端

**Files:**
- Modify: `src-tauri/src/lib.rs`、`src-tauri/src/llm_client.rs`、`src-tauri/src/models.rs`

说明：新前端不再调用旧命令；删除以保持精简。逐项删除并用编译器确认无残留引用。

- [ ] **Step 1: 删除旧命令与注册**

在 `lib.rs` 删除这 5 个 `#[tauri::command]` 函数及其在 `generate_handler![` 列表中的登记项：`start_mock_interview`、`submit_mock_answer`、`submit_mock_follow_up`、`record_skipped_question`、`finish_mock_interview`。同时删除仅被它们使用的常量 `SKIPPED_MARK`（若存在）。

- [ ] **Step 2: 删除仅旧命令使用的 llm_client 函数**

在 `llm_client.rs` 删除 `evaluate_mock_interview_answer` 和 `summarize_mock_interview`（确认新代码不调用它们；新代码用的是 `evaluate_interview`）。

- [ ] **Step 3: 删除仅旧命令使用的模型**

在 `models.rs` 删除 `MockInterviewStart`、`MockEvaluation`、`MockInterviewTurn`、`MockInterviewReport`（grep 确认无其它引用）。同步移除 `lib.rs` 顶部 `use crate::models::{...}` 中对它们的导入。

- [ ] **Step 4: 编译 + 测试**

Run: `cd src-tauri && cargo build && cargo test`
Expected: 编译通过、无 unused 警告（旧代码已彻底移除）；测试仍 12 绿。若报"未使用"或"未找到"，按提示清理残留引用。

- [ ] **Step 5: 前端构建确认无残留调用**

Run: `cd "e:/2026_Junior_S2/Rust/Project/ForgeRust" && npm run build`
Expected: 无错误。用 Grep 搜索 `start_mock_interview|submit_mock_answer|submit_mock_follow_up|record_skipped_question|finish_mock_interview` 确认前端无残留。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/llm_client.rs src-tauri/src/models.rs
git commit -m "refactor: 移除被对话式面试取代的旧模拟面试后端"
```

---

## 完成标准（Plan 2B）

- `npm run build` 无错误；`cargo build && cargo test`（12 绿）无警告。
- 模拟面试页：上传 PDF → 解析出候选人/技术栈/项目并可删误识别项 → 设两环节轮数 → 开始 → 面试官流式提问、多轮对话、项目/八股环节徽章切换 → 结束生成多维雷达复盘 + 对话回放。
- 旧随机抽题式模拟面试后端已移除，代码精简自洽。
- ⚠️ GUI 交互需用户 `npm run tauri dev` 亲自验证（无界面环境无法点测流式与 PDF）。

---

## 自检备注（写计划时已核对）

- 事件名 `interview-token`、负载键 `interviewId`/`chunk` 与 Plan 2A 后端一致。
- 命令参数 camelCase：`parse_resume({rawText})`、`start_interview({resumeId,projectCap,fundamentalCap})`、`interview_respond({interviewId,answer})`、`finish_interview({interviewId})`——与后端 snake_case 形参经 Tauri 自动映射对应。
- `start_interview` 返回元组 → 前端 `const [id, turn] = await invoke(...)` 解构。
- 流式渲染：`loading` 时显示 `streamingText`（实时累积），命令 resolve 后用返回 `message` 定稿到 `currentPrompt`，并 `resetStream()`。
- PDF 走 webview `<input type=file>`，不依赖 Tauri fs 插件。
