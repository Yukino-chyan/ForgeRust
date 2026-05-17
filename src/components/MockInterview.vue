<script setup lang="ts">
import { computed, inject, onMounted, ref, type Ref } from "vue";
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

const hasApiKey = computed(() => !!apiKey.value.trim());
const currentQuestion = computed(() => questions.value[currentIndex.value]);
const progressText = computed(() =>
  questions.value.length ? `${currentIndex.value + 1} / ${questions.value.length}` : "0 / 0"
);
const canStart = computed(() => hasApiKey.value && selectedTags.value.length > 0 && count.value > 0);

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
    stage.value = "interview";
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
    currentEvaluation.value = await invoke<MockEvaluation>("submit_mock_answer", {
      interviewId: interviewId.value,
      questionId: currentQuestion.value.id,
      userAnswer: answer.value,
    });
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
    finishedTurns.value = [...finishedTurns.value, currentEvaluation.value];
    if (currentIndex.value + 1 >= questions.value.length) {
      await finishInterview();
      return;
    }
    currentIndex.value += 1;
    answer.value = "";
    followUpAnswer.value = "";
    currentEvaluation.value = null;
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function finishInterview() {
  if (!interviewId.value) return;
  report.value = await invoke<MockReport>("finish_mock_interview", {
    interviewId: interviewId.value,
  });
  stage.value = "report";
}

function reset() {
  stage.value = "setup";
  interviewId.value = null;
  questions.value = [];
  currentIndex.value = 0;
  answer.value = "";
  followUpAnswer.value = "";
  currentEvaluation.value = null;
  finishedTurns.value = [];
  report.value = null;
  errorMsg.value = "";
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
        <p class="fr-page-subtitle">每题一次回答和一次追问，结束后生成面试复盘。</p>
      </div>
      <span class="mode-chip">文本 MVP</span>
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
      <div class="interview-top">
        <span class="fr-chip">第 {{ progressText }} 题</span>
        <span class="fr-chip fr-chip-accent">{{ currentQuestion?.tags }}</span>
      </div>

      <div class="question-card">
        <div class="interviewer">
          <Icon name="MessagesSquare" :size="18" />
          <span>面试官</span>
        </div>
        <p class="question-text">{{ currentQuestion?.content }}</p>
      </div>

      <div class="answer-card">
        <label>你的回答</label>
        <textarea
          v-model="answer"
          class="fr-input answer-input"
          rows="7"
          :disabled="!!currentEvaluation || loading"
          placeholder="像真实面试一样组织你的回答：先结论，再展开关键点。"
        ></textarea>
        <button class="fr-btn fr-btn-primary" :disabled="loading || !!currentEvaluation || !answer.trim()" @click="submitAnswer">
          <Icon name="Send" :size="14" />
          <span>{{ loading ? "评估中..." : "提交回答" }}</span>
        </button>
      </div>

      <div v-if="currentEvaluation" class="feedback-card">
        <div class="feedback-head">
          <span :class="['score', scoreClass(currentEvaluation.score)]">{{ currentEvaluation.score }}</span>
          <strong>AI 点评</strong>
        </div>
        <p>{{ currentEvaluation.comment }}</p>
        <div class="follow-up">
          <label>追问：{{ currentEvaluation.follow_up }}</label>
          <textarea
            v-model="followUpAnswer"
            class="fr-input answer-input"
            rows="4"
            placeholder="补充回答追问，可简短但要抓住关键。"
          ></textarea>
        </div>
        <button class="fr-btn fr-btn-primary" :disabled="loading" @click="nextQuestion">
          <span>{{ currentIndex + 1 >= questions.length ? "完成并生成报告" : "进入下一题" }}</span>
          <Icon name="ArrowRight" :size="14" />
        </button>
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
.interview-top {
  display: flex;
  gap: var(--sp-2);
}
.question-card, .answer-card, .feedback-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  padding: var(--sp-6);
  box-shadow: var(--shadow-sm);
}
.interviewer {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  color: var(--accent);
  font-size: var(--fs-13);
  font-weight: var(--fw-medium);
  margin-bottom: var(--sp-3);
}
.question-text {
  font-size: var(--fs-16);
  line-height: 1.7;
  color: var(--text);
}
.answer-card, .feedback-card {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
}
.answer-card label, .follow-up label {
  font-size: var(--fs-12);
  color: var(--text-muted);
  font-weight: var(--fw-medium);
}
.answer-input {
  resize: vertical;
  line-height: 1.6;
}
.feedback-head {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
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
.follow-up {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  padding-top: var(--sp-3);
  border-top: 1px solid var(--border);
}
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
