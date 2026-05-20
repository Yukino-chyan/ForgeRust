<script setup lang="ts">
import { computed, inject, nextTick, onMounted, onUnmounted, ref, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Icon from "./ui/Icon.vue";

interface Question {
  id: number;
  question_type: string;
  content: string;
  options: string | null;
  tags: string;
  difficulty: number;
  standard_answer: string;
  explanation: string;
}

interface MockInterviewStart {
  interview_id: number;
  questions: Question[];
}

interface MockEvaluation {
  turn_id: number;
  score: number;
  comment: string;
  follow_up: string;
}

interface MockTurn {
  id: number;
  interview_id: number;
  question_id: number;
  question_content: string;
  user_answer: string;
  ai_comment: string;
  follow_up: string;
  follow_up_answer: string;
  score: number;
  created_at: string;
}

interface MockReport {
  interview_id: number;
  average_score: number;
  summary: string;
  turns: MockTurn[];
}

const apiKey = inject<Ref<string>>("apiKey", ref(""));

const stage = ref<"setup" | "interview" | "report">("setup");
const tags = ref<string[]>([]);
const tagCounts = ref<Record<string, number>>({});
const selectedTags = ref<string[]>([]);
const count = ref(5);
const difficulty = ref(3);
const interviewId = ref<number | null>(null);
const questions = ref<Question[]>([]);
const currentIndex = ref(0);
const answer = ref("");
const followUpAnswer = ref("");
const currentEvaluation = ref<MockEvaluation | null>(null);
const finishedTurns = ref<MockEvaluation[]>([]);
const report = ref<MockReport | null>(null);
const loading = ref(false);
const errorMsg = ref("");

// 对话气泡历史（不含分数，面试中可往回翻）
interface ChatMessage {
  role: "interviewer" | "candidate";
  text: string;
}
const history = ref<ChatMessage[]>([]);
const transcriptRef = ref<HTMLElement | null>(null);

// 面试官当前提问的打字机状态
const promptText = ref("");
const promptShown = ref(0);
let typeTimer: ReturnType<typeof setInterval> | null = null;

// 计时器
const elapsed = ref(0);
let clockTimer: ReturnType<typeof setInterval> | null = null;

// 退出面试的二次确认
const confirmingExit = ref(false);

const hasApiKey = computed(() => !!apiKey.value.trim());
const currentQuestion = computed(() => questions.value[currentIndex.value]);
const canStart = computed(() => hasApiKey.value && selectedTags.value.length > 0 && count.value > 0);

// 进入追问阶段：已提交本题主回答、拿到面试官追问
const isFollowUpPhase = computed(() => currentEvaluation.value !== null);
const promptLabel = computed(() => (isFollowUpPhase.value ? "追问" : "面试官提问"));
const promptDisplay = computed(() => promptText.value.slice(0, promptShown.value));
const isTyping = computed(() => promptShown.value < promptText.value.length);
const isLastQuestion = computed(() => currentIndex.value + 1 >= questions.value.length);
const elapsedText = computed(() => {
  const m = Math.floor(elapsed.value / 60).toString().padStart(2, "0");
  const s = (elapsed.value % 60).toString().padStart(2, "0");
  return `${m}:${s}`;
});

// 当前作答框：主回答阶段绑 answer，追问阶段绑 followUpAnswer
const currentInput = computed({
  get: () => (isFollowUpPhase.value ? followUpAnswer.value : answer.value),
  set: (v: string) => {
    if (isFollowUpPhase.value) followUpAnswer.value = v;
    else answer.value = v;
  },
});

function typePrompt(text: string) {
  if (typeTimer) clearInterval(typeTimer);
  promptText.value = text;
  promptShown.value = 0;
  typeTimer = setInterval(() => {
    if (promptShown.value < promptText.value.length) {
      promptShown.value += 1;
    } else if (typeTimer) {
      clearInterval(typeTimer);
      typeTimer = null;
    }
  }, 28);
}

function startClock() {
  stopClock();
  elapsed.value = 0;
  clockTimer = setInterval(() => {
    elapsed.value += 1;
  }, 1000);
}
function stopClock() {
  if (clockTimer) {
    clearInterval(clockTimer);
    clockTimer = null;
  }
}

function pushMessage(role: ChatMessage["role"], text: string) {
  history.value = [...history.value, { role, text }];
  nextTick(() => {
    const el = transcriptRef.value;
    if (el) el.scrollTop = el.scrollHeight;
  });
}

onUnmounted(() => {
  stopClock();
  if (typeTimer) clearInterval(typeTimer);
});

async function loadTags() {
  const [allTags, counts] = await Promise.all([
    invoke<string[]>("get_all_tags"),
    invoke<Record<string, number>>("get_tag_counts"),
  ]);
  tags.value = allTags;
  tagCounts.value = counts;
}

onMounted(loadTags);

function toggleTag(tag: string) {
  if (selectedTags.value.includes(tag)) {
    selectedTags.value = selectedTags.value.filter((item) => item !== tag);
  } else {
    selectedTags.value = [...selectedTags.value, tag];
  }
}

async function startInterview() {
  loading.value = true;
  errorMsg.value = "";
  try {
    const started = await invoke<MockInterviewStart>("start_mock_interview", {
      tags: selectedTags.value,
      count: count.value,
      difficulty: difficulty.value,
    });
    interviewId.value = started.interview_id;
    questions.value = started.questions;
    currentIndex.value = 0;
    answer.value = "";
    followUpAnswer.value = "";
    currentEvaluation.value = null;
    finishedTurns.value = [];
    report.value = null;
    history.value = [];
    stage.value = "interview";
    startClock();
    typePrompt(started.questions[0]?.content ?? "");
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function submitAnswer() {
  if (!interviewId.value || !currentQuestion.value || !answer.value.trim()) return;
  loading.value = true;
  errorMsg.value = "";
  try {
    const evaluation = await invoke<MockEvaluation>("submit_mock_answer", {
      interviewId: interviewId.value,
      questionId: currentQuestion.value.id,
      userAnswer: answer.value,
    });
    // 主问答沉入对话历史，面试官转为追问（全程不暴露分数/点评）
    pushMessage("interviewer", currentQuestion.value.content);
    pushMessage("candidate", answer.value.trim());
    currentEvaluation.value = evaluation;
    typePrompt(evaluation.follow_up);
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function nextQuestion() {
  if (!currentEvaluation.value) return;
  loading.value = true;
  errorMsg.value = "";
  try {
    await invoke("submit_mock_follow_up", {
      turnId: currentEvaluation.value.turn_id,
      followUpAnswer: followUpAnswer.value,
    });
    pushMessage("interviewer", currentEvaluation.value.follow_up);
    pushMessage("candidate", followUpAnswer.value.trim() || "（未补充）");
    finishedTurns.value = [...finishedTurns.value, currentEvaluation.value];
    if (isLastQuestion.value) {
      await finishInterview();
      return;
    }
    currentIndex.value += 1;
    answer.value = "";
    followUpAnswer.value = "";
    currentEvaluation.value = null;
    typePrompt(currentQuestion.value?.content ?? "");
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function skipQuestion() {
  if (loading.value || isFollowUpPhase.value || !currentQuestion.value || !interviewId.value) return;
  const skipped = currentQuestion.value;
  loading.value = true;
  errorMsg.value = "";
  try {
    // 跳过的题也记一条 0 分 turn，保证面试报告统计准确
    await invoke("record_skipped_question", {
      interviewId: interviewId.value,
      questionId: skipped.id,
      questionContent: skipped.content,
    });
    pushMessage("interviewer", skipped.content);
    pushMessage("candidate", "（跳过此题）");
    if (isLastQuestion.value) {
      await finishInterview();
      return;
    }
    currentIndex.value += 1;
    answer.value = "";
    typePrompt(currentQuestion.value?.content ?? "");
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function finishInterview() {
  if (!interviewId.value) return;
  stopClock();
  report.value = await invoke<MockReport>("finish_mock_interview", {
    interviewId: interviewId.value,
  });
  stage.value = "report";
}

function reset() {
  stopClock();
  if (typeTimer) clearInterval(typeTimer);
  stage.value = "setup";
  interviewId.value = null;
  questions.value = [];
  currentIndex.value = 0;
  answer.value = "";
  followUpAnswer.value = "";
  currentEvaluation.value = null;
  finishedTurns.value = [];
  report.value = null;
  history.value = [];
  promptText.value = "";
  promptShown.value = 0;
  confirmingExit.value = false;
  errorMsg.value = "";
}

function requestExit() {
  confirmingExit.value = true;
}
function cancelExit() {
  confirmingExit.value = false;
}
function confirmExit() {
  reset(); // 回到选考点页，本场进度不保存
}

function scoreClass(score: number) {
  if (score >= 80) return "good";
  if (score >= 60) return "warn";
  return "bad";
}
</script>

<template>
  <div class="fr-page mock-page">
    <header class="mock-head">
      <div>
        <h1 class="fr-page-title">模拟面试</h1>
        <p class="fr-page-subtitle">与 AI 面试官一问一答，全程沉浸，结束后生成面试复盘。</p>
      </div>
      <span v-if="stage === 'interview'" class="mode-chip">面试进行中</span>
    </header>

    <section v-if="stage === 'setup'" class="fr-card setup-panel">
      <div class="field">
        <label>考点范围</label>
        <div class="tag-grid">
          <button
            v-for="tag in tags"
            :key="tag"
            :class="['tag-pill', { active: selectedTags.includes(tag), empty: (tagCounts[tag] ?? 0) === 0 }]"
            :disabled="(tagCounts[tag] ?? 0) === 0"
            @click="toggleTag(tag)"
          >
            {{ tag }}
            <span class="count">{{ tagCounts[tag] ?? 0 }}</span>
          </button>
        </div>
      </div>

      <div class="setup-grid">
        <div class="field">
          <label>题量</label>
          <input v-model.number="count" class="fr-input" type="number" min="1" max="20" />
        </div>
        <div class="field">
          <label>最高难度</label>
          <select v-model.number="difficulty" class="fr-input">
            <option :value="1">1 - 入门</option>
            <option :value="2">2 - 基础</option>
            <option :value="3">3 - 中等</option>
            <option :value="4">4 - 进阶</option>
            <option :value="5">5 - 深入</option>
          </select>
        </div>
      </div>

      <div v-if="!hasApiKey" class="notice">
        <Icon name="AlertTriangle" :size="16" />
        <span>模拟面试需要先在设置页配置 API Key。</span>
      </div>
      <p v-if="errorMsg" class="error-msg">{{ errorMsg }}</p>

      <div class="actions">
        <button class="fr-btn fr-btn-primary" :disabled="!canStart || loading" @click="startInterview">
          <Icon name="MessageSquare" :size="14" />
          <span>{{ loading ? "准备中..." : "开始模拟面试" }}</span>
        </button>
      </div>
    </section>

    <section v-else-if="stage === 'interview'" class="interview-panel">
      <!-- 退出面试 -->
      <div class="interview-bar">
        <div v-if="confirmingExit" class="exit-confirm">
          <span>确定退出？本场进度不会保存。</span>
          <button class="fr-btn fr-btn-ghost" @click="cancelExit">取消</button>
          <button class="fr-btn fr-btn-danger" @click="confirmExit">确定退出</button>
        </div>
        <button v-else class="fr-btn fr-btn-ghost" :disabled="loading" @click="requestExit">
          <Icon name="X" :size="14" />
          <span>退出面试</span>
        </button>
      </div>

      <!-- 顶部视频条 -->
      <div class="video-bar">
        <div class="video-tile interviewer-tile">
          <span class="rec-dot"><span class="dot"></span>录制中</span>
          <span class="clock fr-mono">{{ elapsedText }}</span>
          <div class="avatar">AI</div>
          <span class="tile-label">AI 面试官</span>
        </div>
        <div class="video-tile candidate-tile">
          <Icon name="VideoOff" :size="18" />
          <span class="cam-hint">摄像头未开启</span>
          <span class="tile-label">你</span>
        </div>
      </div>

      <!-- 对话历史（可往回翻，不含分数） -->
      <div v-if="history.length" ref="transcriptRef" class="transcript">
        <div
          v-for="(msg, i) in history"
          :key="i"
          :class="['bubble-row', msg.role]"
        >
          <div class="bubble">{{ msg.text }}</div>
        </div>
      </div>

      <!-- 面试官当前提问 -->
      <div class="prompt-box">
        <span class="prompt-label">{{ promptLabel }}</span>
        <p class="prompt-text">
          {{ promptDisplay }}<span v-if="isTyping" class="caret">▌</span>
        </p>
        <span v-if="loading" class="thinking">
          <Icon name="Loader" :size="13" /> 面试官思考中…
        </span>
      </div>

      <!-- 作答区 -->
      <div class="answer-card">
        <label>{{ isFollowUpPhase ? "回答追问" : "你的回答" }}</label>
        <textarea
          v-model="currentInput"
          class="fr-input answer-input"
          :rows="isFollowUpPhase ? 4 : 7"
          :disabled="loading"
          :placeholder="isFollowUpPhase
            ? '补充回答追问，可简短但要抓住关键。'
            : '像真实面试一样组织你的回答：先结论，再展开关键点。'"
        ></textarea>
        <div class="answer-actions">
          <button
            v-if="!isFollowUpPhase"
            class="fr-btn fr-btn-ghost"
            :disabled="loading"
            @click="skipQuestion"
          >
            跳过
          </button>
          <button
            v-if="!isFollowUpPhase"
            class="fr-btn fr-btn-primary"
            :disabled="loading || !answer.trim()"
            @click="submitAnswer"
          >
            <Icon name="Send" :size="14" />
            <span>{{ loading ? "提交中..." : "回答" }}</span>
          </button>
          <button
            v-else
            class="fr-btn fr-btn-primary"
            :disabled="loading"
            @click="nextQuestion"
          >
            <span>{{ isLastQuestion ? "结束面试并生成报告" : "进入下一题" }}</span>
            <Icon name="ArrowRight" :size="14" />
          </button>
        </div>
      </div>

      <p v-if="errorMsg" class="error-msg">{{ errorMsg }}</p>
    </section>

    <section v-else-if="report" class="report-panel">
      <div class="fr-card report-hero">
        <div>
          <span class="report-label">面试报告</span>
          <h2>{{ report.average_score.toFixed(1) }}</h2>
          <p>{{ report.summary }}</p>
        </div>
        <button class="fr-btn fr-btn-ghost" @click="reset">
          <Icon name="RotateCcw" :size="14" />
          <span>再来一场</span>
        </button>
      </div>

      <div class="turn-list">
        <article v-for="turn in report.turns" :key="turn.id" class="fr-card turn-card">
          <div class="turn-head">
            <strong>{{ turn.question_content }}</strong>
            <span :class="['score small', scoreClass(turn.score)]">{{ turn.score }}</span>
          </div>
          <p><b>回答：</b>{{ turn.user_answer }}</p>
          <p><b>点评：</b>{{ turn.ai_comment }}</p>
          <p><b>追问：</b>{{ turn.follow_up }}</p>
          <p><b>追问回答：</b>{{ turn.follow_up_answer || "未填写" }}</p>
        </article>
      </div>
    </section>
  </div>
</template>

<style scoped>
.mock-page { max-width: var(--content-max); margin: 0 auto; }
.mock-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--sp-4);
}
.mode-chip {
  font-size: var(--fs-12);
  color: var(--accent);
  background: var(--accent-soft);
  border-radius: 999px;
  padding: 4px 10px;
}
.setup-panel, .interview-panel, .report-panel {
  display: flex;
  flex-direction: column;
  gap: var(--sp-4);
}
.field {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
}
.field label {
  font-size: var(--fs-12);
  color: var(--text-muted);
  font-weight: var(--fw-medium);
}
.tag-grid {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.tag-pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border-radius: 999px;
  font-size: var(--fs-12);
  color: var(--text-muted);
  background: var(--surface);
  border: 1px solid var(--border);
}
.tag-pill.active {
  color: var(--accent);
  background: var(--accent-soft);
  border-color: transparent;
}
.tag-pill.empty {
  opacity: 0.45;
  cursor: not-allowed;
}
.count { color: var(--text-subtle); }
.setup-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--sp-4);
}
.notice, .error-msg {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  font-size: var(--fs-13);
}
.notice { color: var(--warning); }
.error-msg { color: var(--danger); }
.actions { display: flex; justify-content: flex-end; }
/* ── 退出面试 ── */
.interview-bar {
  display: flex;
  justify-content: flex-end;
}
.exit-confirm {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  font-size: var(--fs-13);
  color: var(--text-muted);
}

/* ── 顶部视频条 ── */
.video-bar {
  display: flex;
  gap: var(--sp-3);
}
.video-tile {
  position: relative;
  height: 120px;
  border-radius: var(--radius-lg);
  border: 1px solid var(--border);
  background: var(--surface);
  box-shadow: var(--shadow-sm);
  display: flex;
  align-items: center;
  justify-content: center;
}
.interviewer-tile { flex: 1; }
.candidate-tile {
  width: 160px;
  flex-shrink: 0;
  flex-direction: column;
  gap: 4px;
  background: var(--surface-2);
  color: var(--text-subtle);
}
.candidate-tile .cam-hint { font-size: var(--fs-12); }
.avatar {
  width: 46px;
  height: 46px;
  border-radius: 50%;
  background: var(--accent);
  color: var(--text-on-accent);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: var(--fs-16);
  font-weight: var(--fw-semibold);
}
.rec-dot {
  position: absolute;
  top: 9px;
  left: 11px;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: var(--fs-12);
  color: var(--danger);
}
.rec-dot .dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--danger);
  animation: rec-pulse 1.4s ease-in-out infinite;
}
@keyframes rec-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.3; }
}
.clock {
  position: absolute;
  top: 9px;
  right: 11px;
  font-size: var(--fs-12);
  color: var(--text-subtle);
}
.tile-label {
  position: absolute;
  bottom: 9px;
  left: 11px;
  font-size: var(--fs-12);
  color: var(--text-muted);
}

/* ── 对话历史气泡 ── */
.transcript {
  max-height: 240px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  padding: var(--sp-1);
}
.bubble-row { display: flex; }
.bubble-row.interviewer { justify-content: flex-start; }
.bubble-row.candidate { justify-content: flex-end; }
.bubble {
  max-width: 78%;
  padding: 8px 12px;
  font-size: var(--fs-13);
  line-height: 1.6;
  border-radius: var(--radius-lg);
}
.bubble-row.interviewer .bubble {
  background: var(--accent-soft);
  color: var(--text);
  border-bottom-left-radius: var(--radius-sm);
}
.bubble-row.candidate .bubble {
  background: var(--surface-2);
  color: var(--text);
  border: 1px solid var(--border);
  border-bottom-right-radius: var(--radius-sm);
}

/* ── 面试官当前提问 ── */
.prompt-box {
  background: var(--accent-soft);
  border-left: 3px solid var(--accent);
  border-radius: var(--radius-md);
  padding: var(--sp-3) var(--sp-4);
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
}
.prompt-label {
  font-size: var(--fs-12);
  font-weight: var(--fw-medium);
  color: var(--accent);
}
.prompt-text {
  font-size: var(--fs-16);
  line-height: 1.7;
  color: var(--text);
}
.caret {
  opacity: 0.45;
  animation: caret-blink 1s step-end infinite;
}
@keyframes caret-blink {
  50% { opacity: 0; }
}
.thinking {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: var(--fs-12);
  color: var(--text-muted);
}

/* ── 作答区 ── */
.answer-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  padding: var(--sp-6);
  box-shadow: var(--shadow-sm);
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
}
.answer-card label {
  font-size: var(--fs-12);
  color: var(--text-muted);
  font-weight: var(--fw-medium);
}
.answer-input {
  resize: vertical;
  line-height: 1.6;
}
.answer-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--sp-2);
}
.score {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 44px;
  height: 44px;
  border-radius: var(--radius-md);
  font-family: var(--font-mono);
  font-weight: var(--fw-semibold);
}
.score.small {
  width: 36px;
  height: 30px;
  font-size: var(--fs-13);
}
.score.good { color: var(--success); background: var(--success-soft); }
.score.warn { color: var(--warning); background: var(--warning-soft); }
.score.bad { color: var(--danger); background: var(--danger-soft); }
.report-hero {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--sp-4);
}
.report-label {
  font-size: var(--fs-12);
  color: var(--text-muted);
}
.report-hero h2 {
  font-size: var(--fs-28);
  color: var(--accent);
  margin: var(--sp-1) 0;
}
.report-hero p {
  color: var(--text-muted);
  line-height: 1.6;
}
.turn-list {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
}
.turn-card {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
}
.turn-head {
  display: flex;
  justify-content: space-between;
  gap: var(--sp-4);
}
.turn-card p {
  color: var(--text-muted);
  line-height: 1.6;
  font-size: var(--fs-13);
}
@media (max-width: 760px) {
  .setup-grid, .report-hero {
    grid-template-columns: 1fr;
    flex-direction: column;
  }
}
</style>
