<script setup lang="ts">
import { computed, inject, nextTick, onUnmounted, ref, shallowRef, watch, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Icon from "./ui/Icon.vue";
import { extractPdfText } from "../utils/pdfText";
import { useInterviewStream } from "../composables/useInterviewStream";

import * as echarts from "echarts/core";
import { RadarChart } from "echarts/charts";
import { TooltipComponent } from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
echarts.use([RadarChart, TooltipComponent, CanvasRenderer]);

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
const radarEl = ref<HTMLElement | null>(null);
const radarChart = shallowRef<echarts.ECharts | null>(null);

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

onUnmounted(() => { stopClock(); stop(); radarChart.value?.dispose(); });

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
        <div ref="radarEl" class="radar"></div>
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
.radar { width: 100%; height: 280px; }

.icon-btn { width: 26px; height: 26px; border-radius: var(--radius-sm); color: var(--text-subtle); display: inline-flex; align-items: center; justify-content: center; }
.icon-btn.danger:hover { color: var(--danger); background: var(--danger-soft); }

@media (max-width: 760px) {
  .report-hero, .video-bar { flex-direction: column; }
  .candidate-tile { width: 100%; height: 80px; flex-direction: row; }
}
</style>
