<script setup lang="ts">
import { ref, computed, inject, onMounted, onUnmounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Icon from "./ui/Icon.vue";

interface AiResponse {
  standard_answer: string;
  explanation: string;
  is_correct: boolean | null;
  ai_comment: string;
  score: number;
}
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
interface TrainingResult {
  question: Question;
  userAnswer: string;
  evaluation: AiResponse;
  timeSpent: number;
  skipped: boolean;
  manuallyAdded: boolean;
}

const props = defineProps<{ wrongPracticeIds?: number[]; isActive?: boolean }>();
const emit = defineEmits<{
  consumed: [];
  stateChange: [state: "setup" | "interview" | "summary"];
}>();

const hasPending = !!(props.wrongPracticeIds && props.wrongPracticeIds.length > 0);
const appState = ref<"setup" | "interview" | "summary">(hasPending ? "interview" : "setup");
const isPreloading = ref(hasPending);

const sessionSaved = ref(false);
const selectedTags = ref<string[]>([]);
const countPerTag = ref(2);
const useAI = ref(true);
const questionList = ref<Question[]>([]);
const currentIndex = ref(0);
const userAnswer = ref("");
const multiAnswers = ref<string[]>([]);
const aiResult = ref<AiResponse | null>(null);
const isLoading = ref(false);
const trainingResults = ref<TrainingResult[]>([]);
const showExitConfirm = ref(false);

const tags = ref<string[]>([]);
const tagCounts = ref<Record<string, number>>({});
const injectedApiKey = inject<ReturnType<typeof ref<string>>>("apiKey");
const hasApiKey = computed(() => !!injectedApiKey?.value?.trim());

watch(appState, (s) => emit("stateChange", s), { immediate: true });

watch(
  () => props.isActive,
  async (active) => {
    if (!active && appState.value === "summary") saveTrainingSession();
    if (active && appState.value === "setup") {
      try {
        [tags.value, tagCounts.value] = await Promise.all([
          invoke<string[]>("get_all_tags"),
          invoke<Record<string, number>>("get_tag_counts"),
        ]);
      } catch (e) {
        console.error("刷新标签失败", e);
      }
    }
  }
);

watch(
  () => props.wrongPracticeIds,
  async (ids) => {
    if (!ids || ids.length === 0) return;
    currentIndex.value = 0;
    userAnswer.value = "";
    multiAnswers.value = [];
    aiResult.value = null;
    trainingResults.value = [];
    showExitConfirm.value = false;
    sessionSaved.value = false;
    questionList.value = [];
    appState.value = "interview";
    isPreloading.value = true;
    startTimer();
    try {
      questionList.value = await invoke<Question[]>("generate_interview_from_ids", { questionIds: ids });
      emit("consumed");
    } catch (e) {
      appState.value = "setup";
      alert(e);
    } finally {
      isPreloading.value = false;
    }
  },
  { immediate: true }
);

onMounted(async () => {
  try {
    [tags.value, tagCounts.value] = await Promise.all([
      invoke<string[]>("get_all_tags"),
      invoke<Record<string, number>>("get_tag_counts"),
    ]);
  } catch (e) {
    console.error("加载标签失败", e);
  }
});

const elapsedSeconds = ref(0);
let timerInterval: ReturnType<typeof setInterval> | null = null;

const formattedTime = computed(() => {
  const m = Math.floor(elapsedSeconds.value / 60);
  const s = elapsedSeconds.value % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
});

function startTimer() {
  elapsedSeconds.value = 0;
  if (timerInterval) clearInterval(timerInterval);
  timerInterval = setInterval(() => {
    elapsedSeconds.value++;
  }, 1000);
}

function stopTimer(): number {
  if (timerInterval) {
    clearInterval(timerInterval);
    timerInterval = null;
  }
  return elapsedSeconds.value;
}

onUnmounted(() => {
  if (timerInterval) clearInterval(timerInterval);
});

const canSubmit = computed(() => {
  if (!currentQuestion.value) return false;
  if (currentQuestion.value.question_type === "MULTI") return multiAnswers.value.length > 0;
  return !!userAnswer.value;
});

const expectedCount = computed(() =>
  selectedTags.value.reduce(
    (sum, tag) => sum + Math.min(countPerTag.value, tagCounts.value[tag] ?? 0),
    0
  )
);

const insufficientTags = computed(() =>
  selectedTags.value.filter((tag) => (tagCounts.value[tag] ?? 0) < countPerTag.value)
);

const currentQuestion = computed(() => questionList.value[currentIndex.value]);
const currentOptions = computed(() => {
  if (!currentQuestion.value || !currentQuestion.value.options) return [];
  try {
    return JSON.parse(currentQuestion.value.options);
  } catch (e) {
    return [];
  }
});
const progress = computed(() =>
  questionList.value.length
    ? Math.round((currentIndex.value / questionList.value.length) * 100)
    : 0
);

const totalCount = computed(() => trainingResults.value.length);
const skippedCount = computed(() => trainingResults.value.filter((r) => r.skipped).length);
const attemptedResults = computed(() => trainingResults.value);
const correctCount = computed(
  () =>
    attemptedResults.value.filter((r) => {
      if (r.evaluation.score === -1) return false;
      return r.evaluation.is_correct !== null
        ? r.evaluation.is_correct
        : r.evaluation.score >= 60;
    }).length
);
const scoredResults = computed(() =>
  attemptedResults.value.filter((r) => r.evaluation.score >= 0)
);
const averageScore = computed(() => {
  if (scoredResults.value.length === 0) return "—";
  return Math.round(
    scoredResults.value.reduce((a, r) => a + r.evaluation.score, 0) / scoredResults.value.length
  );
});
const accuracyRate = computed(() =>
  totalCount.value === 0 ? 0 : Math.round((correctCount.value / totalCount.value) * 100)
);

const difficultyLabel = (d: number) => ["", "入门", "简单", "中等", "困难", "专家"][d] ?? "—";
const typeLabel = (t: string) => ({ SINGLE: "单选", MULTI: "多选", ESSAY: "简答" }[t] ?? t);

function parseOption(opt: string): { letter: string; text: string } {
  const match = opt.match(/^[（(]?([A-Za-z])[)）.、。：:\s]+(.+)$/s);
  if (match) return { letter: match[1].toUpperCase(), text: match[2].trim() };
  return { letter: opt.charAt(0).toUpperCase(), text: opt.slice(1).trimStart() };
}

const currentResult = computed(() =>
  trainingResults.value.find((r) => r.question.id === currentQuestion.value?.id)
);

function toggleManualMark() {
  const r = currentResult.value;
  if (r) r.manuallyAdded = !r.manuallyAdded;
}

async function saveTrainingSession() {
  if (sessionSaved.value || trainingResults.value.length === 0) return;
  sessionSaved.value = true;
  try {
    await invoke("save_training_session", {
      records: trainingResults.value.map((r) => ({
        question_id: r.question.id,
        user_answer: r.userAnswer,
        score: r.evaluation.score,
        is_correct: r.evaluation.is_correct,
        skipped: r.skipped,
        manually_added: r.manuallyAdded,
        time_spent: r.timeSpent,
      })),
      tags: selectedTags.value,
    });
  } catch (e) {
    console.error("保存训练记录失败", e);
  }
}

function toggleTag(tag: string) {
  const i = selectedTags.value.indexOf(tag);
  if (i > -1) selectedTags.value.splice(i, 1);
  else selectedTags.value.push(tag);
}

async function startInterview() {
  if (selectedTags.value.length === 0) return alert("请至少选择一个考点。");
  try {
    questionList.value = await invoke("generate_interview", {
      tags: selectedTags.value,
      count: countPerTag.value,
    });
    currentIndex.value = 0;
    userAnswer.value = "";
    multiAnswers.value = [];
    aiResult.value = null;
    trainingResults.value = [];
    showExitConfirm.value = false;
    sessionSaved.value = false;
    appState.value = "interview";
    startTimer();
  } catch (error) {
    alert(error);
  }
}

async function startEvaluation() {
  if (currentQuestion.value?.question_type === "MULTI") {
    userAnswer.value = [...multiAnswers.value].sort().join(",");
  }
  if (!userAnswer.value) return alert("请先给出你的答案。");

  if (!useAI.value && currentQuestion.value?.question_type === "ESSAY") {
    const q = currentQuestion.value;
    const result: AiResponse = {
      standard_answer: q.standard_answer,
      explanation: q.explanation,
      is_correct: null,
      ai_comment: "AI 点评已关闭，请对照标准答案自行评估。",
      score: -1,
    };
    aiResult.value = result;
    trainingResults.value.push({
      question: q,
      userAnswer: userAnswer.value,
      evaluation: result,
      timeSpent: stopTimer(),
      skipped: false,
      manuallyAdded: false,
    });
    return;
  }

  isLoading.value = true;
  try {
    const result: AiResponse = await invoke("evaluate_answer", {
      questionId: currentQuestion.value.id,
      userAnswer: userAnswer.value,
    });
    aiResult.value = result;
    trainingResults.value.push({
      question: currentQuestion.value,
      userAnswer: userAnswer.value,
      evaluation: result,
      timeSpent: stopTimer(),
      skipped: false,
      manuallyAdded: false,
    });
  } catch (error) {
    alert(`系统内部调用报错: ${error}`);
  } finally {
    isLoading.value = false;
  }
}

function skipQuestion() {
  const q = currentQuestion.value;
  const spent = stopTimer();
  trainingResults.value.push({
    question: q,
    userAnswer: "",
    evaluation: {
      standard_answer: q.standard_answer,
      explanation: q.explanation,
      is_correct: false,
      ai_comment: "",
      score: 0,
    },
    timeSpent: spent,
    skipped: true,
    manuallyAdded: false,
  });
  if (currentIndex.value < questionList.value.length - 1) {
    currentIndex.value++;
    userAnswer.value = "";
    multiAnswers.value = [];
    aiResult.value = null;
    startTimer();
  } else {
    appState.value = "summary";
  }
}

function nextQuestion() {
  if (currentIndex.value < questionList.value.length - 1) {
    currentIndex.value++;
    userAnswer.value = "";
    multiAnswers.value = [];
    aiResult.value = null;
    startTimer();
  } else {
    appState.value = "summary";
  }
}

function exitTraining() {
  showExitConfirm.value = true;
}
function confirmExit() {
  stopTimer();
  showExitConfirm.value = false;
  restartTraining();
}
function restartTraining() {
  saveTrainingSession();
  stopTimer();
  appState.value = "setup";
  selectedTags.value = [];
  trainingResults.value = [];
  userAnswer.value = "";
  multiAnswers.value = [];
  aiResult.value = null;
  showExitConfirm.value = false;
}

function isCorrectish(r: TrainingResult): boolean {
  if (r.skipped) return false;
  if (r.evaluation.is_correct !== null) return r.evaluation.is_correct;
  return r.evaluation.score >= 60;
}

function formatTime(sec: number) {
  return `${Math.floor(sec / 60)}:${String(sec % 60).padStart(2, "0")}`;
}
</script>

<template>
  <div class="training-root">
    <!-- ───── Setup 页 ───── -->
    <Transition name="page">
      <section v-if="appState === 'setup'" class="page setup" key="setup">
        <div class="setup-card fr-card">
          <header class="setup-head">
            <h1>题库专项训练</h1>
            <p>选择考点，按数量生成本次训练。</p>
          </header>

          <div class="block">
            <div class="block-head">
              <span class="block-label">选择考点</span>
              <span v-if="selectedTags.length" class="block-aside">
                已选 <strong class="fr-mono">{{ selectedTags.length }}</strong> 个
              </span>
            </div>
            <div class="tag-grid">
              <button
                v-for="tag in tags"
                :key="tag"
                :class="['tag-pill', { active: selectedTags.includes(tag) }]"
                @click="toggleTag(tag)"
              >
                <span>{{ tag }}</span>
                <span class="tag-count fr-mono">{{ tagCounts[tag] ?? 0 }}</span>
              </button>
            </div>
          </div>

          <div class="grid-2">
            <div class="block">
              <div class="block-head">
                <span class="block-label">每个考点题数</span>
                <span class="block-aside">
                  实际约 <strong class="fr-mono">{{ expectedCount }}</strong> 题
                </span>
              </div>
              <input
                type="number"
                class="fr-input"
                v-model.number="countPerTag"
                min="1"
                max="20"
              />
            </div>

            <div class="block">
              <div class="block-head">
                <span class="block-label">AI 点评</span>
                <span :class="['block-aside', useAI ? 'ok' : 'off']">
                  {{ useAI ? "已开启" : "已关闭" }}
                </span>
              </div>
              <button :class="['toggle', { on: useAI }]" @click="useAI = !useAI">
                <span class="toggle-track">
                  <span class="toggle-thumb"></span>
                </span>
                <span class="toggle-text">
                  {{ useAI ? "简答题将由 AI 评分" : "仅展示标准答案" }}
                </span>
              </button>
            </div>
          </div>

          <div v-if="useAI && !hasApiKey" class="notice notice-danger">
            <Icon name="AlertTriangle" :size="14" />
            <span>已开启 AI 点评，但尚未配置 API Key。请到「设置」中填写。</span>
          </div>

          <div v-if="insufficientTags.length > 0" class="notice notice-warning">
            <Icon name="AlertCircle" :size="14" />
            <span>
              以下考点题目数量不足，将按实际库存出题：
              <span v-for="tag in insufficientTags" :key="tag" class="insufficient-tag">
                {{ tag }}（{{ tagCounts[tag] }} 题）
              </span>
            </span>
          </div>

          <button
            class="fr-btn fr-btn-primary start-btn"
            :disabled="selectedTags.length === 0 || (useAI && !hasApiKey) || expectedCount === 0"
            @click="startInterview"
          >
            <Icon name="Play" :size="14" />
            <span>开始训练</span>
            <span v-if="selectedTags.length" class="start-hint">
              · <span class="fr-mono">{{ expectedCount }}</span> 题
            </span>
          </button>
        </div>
      </section>
    </Transition>

    <!-- ───── 答题页 ───── -->
    <Transition name="page">
      <section v-if="appState === 'interview'" class="page interview" key="interview">
        <div class="progress-track">
          <div class="progress-fill" :style="{ width: progress + '%' }"></div>
        </div>

        <Transition name="confirm">
          <div v-if="showExitConfirm" class="exit-bar">
            <span>确定退出本次训练？已答记录将丢失。</span>
            <div class="exit-actions">
              <button class="fr-btn fr-btn-ghost" @click="showExitConfirm = false">继续训练</button>
              <button class="fr-btn fr-btn-danger" @click="confirmExit">确定退出</button>
            </div>
          </div>
        </Transition>

        <div v-if="isPreloading" class="preload">
          <Icon name="Loader2" :size="20" class="spin" />
          <p>题目加载中</p>
        </div>

        <div v-else class="iv-content">
          <div class="iv-meta">
            <span class="iv-index fr-mono">
              {{ currentIndex + 1 }} / {{ questionList.length }}
            </span>
            <span class="fr-chip fr-chip-accent">{{ currentQuestion?.tags }}</span>
            <span class="fr-chip">{{ typeLabel(currentQuestion?.question_type) }}</span>
            <span class="fr-chip iv-diff">
              {{ difficultyLabel(currentQuestion?.difficulty) }}
            </span>
            <span class="iv-timer fr-mono">
              <Icon name="Clock" :size="13" />
              {{ formattedTime }}
            </span>
            <button class="iv-exit" @click="exitTraining">
              <Icon name="X" :size="14" />
              <span>退出</span>
            </button>
          </div>

          <div class="fr-card q-card">
            <p class="q-text">{{ currentQuestion?.content }}</p>
          </div>

          <div class="fr-card a-card">
            <div v-if="currentQuestion?.question_type === 'SINGLE'" class="options">
              <label
                v-for="opt in currentOptions"
                :key="opt"
                :class="['option', {
                  selected: userAnswer === parseOption(opt).letter,
                  disabled: !!aiResult,
                }]"
              >
                <input
                  type="radio"
                  name="single"
                  :value="parseOption(opt).letter"
                  v-model="userAnswer"
                  :disabled="isLoading || !!aiResult"
                />
                <span class="opt-letter fr-mono">{{ parseOption(opt).letter }}</span>
                <span class="opt-text">{{ parseOption(opt).text }}</span>
              </label>
            </div>

            <div v-else-if="currentQuestion?.question_type === 'MULTI'" class="options">
              <p class="multi-hint">多选题 · 请选择所有正确选项</p>
              <label
                v-for="opt in currentOptions"
                :key="opt"
                :class="['option', {
                  selected: multiAnswers.includes(parseOption(opt).letter),
                  disabled: !!aiResult,
                }]"
              >
                <input
                  type="checkbox"
                  :value="parseOption(opt).letter"
                  v-model="multiAnswers"
                  :disabled="isLoading || !!aiResult"
                />
                <span class="opt-letter fr-mono">{{ parseOption(opt).letter }}</span>
                <span class="opt-text">{{ parseOption(opt).text }}</span>
              </label>
            </div>

            <div v-else-if="currentQuestion?.question_type === 'ESSAY'">
              <textarea
                v-model="userAnswer"
                placeholder="请详细阐述你的回答..."
                :disabled="isLoading || !!aiResult"
                class="essay-input"
              ></textarea>
            </div>

            <div class="a-actions">
              <button
                class="fr-btn fr-btn-ghost"
                :disabled="isLoading || !!aiResult"
                @click="skipQuestion"
              >
                跳过此题
              </button>
              <button
                class="fr-btn fr-btn-primary"
                :disabled="isLoading || !canSubmit || !!aiResult"
                @click="startEvaluation"
              >
                <Icon v-if="isLoading" name="Loader2" :size="14" class="spin" />
                <Icon v-else name="Check" :size="14" />
                <span>{{ isLoading ? "批阅中..." : "提交回答" }}</span>
              </button>
            </div>
          </div>

          <Transition name="result">
            <div v-if="aiResult" class="fr-card result-card">
              <div class="result-top">
                <div
                  v-if="aiResult.score !== -1"
                  :class="['score-badge', aiResult.score >= 60 ? 'pass' : 'fail']"
                >
                  <span class="score-num fr-mono">{{ aiResult.score }}</span>
                  <span class="score-unit">分</span>
                </div>
                <div v-else class="score-badge score-na">未评分</div>

                <div
                  v-if="aiResult.is_correct !== null"
                  :class="['verdict', aiResult.is_correct ? 'correct' : 'wrong']"
                >
                  <Icon :name="aiResult.is_correct ? 'CheckCircle2' : 'XCircle'" :size="16" />
                  <span>{{ aiResult.is_correct ? "回答正确" : "回答错误" }}</span>
                </div>
              </div>

              <div class="result-section">
                <div class="rs-label">AI 点评</div>
                <p class="rs-text">{{ aiResult.ai_comment }}</p>
              </div>

              <div class="result-section">
                <div class="rs-label">标准答案</div>
                <p class="rs-text">{{ aiResult.standard_answer }}</p>
              </div>

              <div class="result-section">
                <div class="rs-label">解析</div>
                <p class="rs-text">{{ aiResult.explanation }}</p>
              </div>

              <div class="result-actions">
                <button
                  :class="['fr-btn', currentResult?.manuallyAdded ? 'fr-btn-primary' : 'fr-btn-ghost']"
                  @click="toggleManualMark"
                >
                  <Icon
                    :name="currentResult?.manuallyAdded ? 'BookmarkCheck' : 'Bookmark'"
                    :size="14"
                  />
                  <span>
                    {{ currentResult?.manuallyAdded ? "已加入错题本" : "加入错题本" }}
                  </span>
                </button>
                <button class="fr-btn fr-btn-primary" @click="nextQuestion">
                  <span>
                    {{ currentIndex < questionList.length - 1 ? "下一题" : "查看报告" }}
                  </span>
                  <Icon name="ArrowRight" :size="14" />
                </button>
              </div>
            </div>
          </Transition>
        </div>
      </section>
    </Transition>

    <!-- ───── 总结页 ───── -->
    <Transition name="page">
      <section v-if="appState === 'summary'" class="page summary" key="summary">
        <div class="sm-wrap">
          <header class="sm-head">
            <div>
              <h2>训练完成</h2>
              <p>本次共 {{ totalCount }} 道题，以下是你的表现。</p>
            </div>
            <button
              class="fr-btn fr-btn-ghost"
              @click="trainingResults.forEach(r => r.manuallyAdded = true)"
            >
              <Icon name="BookmarkPlus" :size="14" />
              <span>全部加入错题本</span>
            </button>
          </header>

          <div class="stats">
            <div class="fr-card stat">
              <div class="stat-label">正确率</div>
              <div :class="['stat-value fr-mono', accuracyRate >= 60 ? 'good' : 'bad']">
                {{ accuracyRate }}<span class="stat-unit">%</span>
              </div>
            </div>
            <div class="fr-card stat">
              <div class="stat-label">答对题数</div>
              <div class="stat-value fr-mono">
                {{ correctCount }}<span class="stat-unit">/{{ totalCount }}</span>
              </div>
            </div>
            <div class="fr-card stat">
              <div class="stat-label">平均分</div>
              <div
                :class="['stat-value fr-mono', averageScore === '—' ? '' : (averageScore as number) >= 60 ? 'good' : 'bad']"
              >
                {{ averageScore }}
              </div>
            </div>
            <div class="fr-card stat">
              <div class="stat-label">跳过题数</div>
              <div :class="['stat-value fr-mono', skippedCount > 0 ? 'bad' : '']">
                {{ skippedCount }}
              </div>
            </div>
          </div>

          <ul class="result-list">
            <li
              v-for="(r, idx) in trainingResults"
              :key="idx"
              :class="['result-row', r.skipped ? 'skipped' : isCorrectish(r) ? 'correct' : 'wrong']"
            >
              <div class="row-left">
                <span class="row-num fr-mono">{{ idx + 1 }}</span>
                <span class="row-status">
                  <Icon
                    v-if="r.skipped"
                    name="SkipForward"
                    :size="16"
                  />
                  <Icon
                    v-else-if="isCorrectish(r)"
                    name="CheckCircle2"
                    :size="16"
                  />
                  <Icon v-else name="XCircle" :size="16" />
                </span>
              </div>

              <div class="row-body">
                <p class="row-q">{{ r.question.content }}</p>
                <div class="row-meta">
                  <span class="fr-chip fr-chip-accent">{{ r.question.tags }}</span>
                  <span class="fr-chip">{{ typeLabel(r.question.question_type) }}</span>
                  <span v-if="r.skipped" class="fr-chip pin">已跳过</span>
                </div>

                <div v-if="r.skipped" class="ans-blocks">
                  <div class="ans-block">
                    <div class="ans-label">标准答案（供参考）</div>
                    <p>{{ r.question.standard_answer }}</p>
                  </div>
                </div>

                <div v-else-if="r.evaluation.is_correct !== null" class="ans-inline">
                  <span class="your-ans">你的答案：<strong class="fr-mono">{{ r.userAnswer }}</strong></span>
                  <span v-if="r.evaluation.is_correct === false" class="std-ans">
                    正确答案：<strong class="fr-mono">{{ r.evaluation.standard_answer }}</strong>
                  </span>
                </div>

                <div v-else class="ans-blocks">
                  <div class="ans-block">
                    <div class="ans-label">你的回答</div>
                    <p>{{ r.userAnswer }}</p>
                  </div>
                  <div class="ans-block">
                    <div class="ans-label">标准答案</div>
                    <p>{{ r.evaluation.standard_answer }}</p>
                  </div>
                </div>

                <p v-if="r.evaluation.ai_comment" class="row-comment">
                  {{ r.evaluation.ai_comment }}
                </p>
              </div>

              <div class="row-right">
                <span :class="['row-score fr-mono', r.evaluation.score >= 60 ? 'good' : 'bad']">
                  {{ r.evaluation.score < 0 ? "—" : r.evaluation.score }}
                </span>
                <span class="row-time fr-mono">{{ formatTime(r.timeSpent) }}</span>
                <button
                  class="row-mark"
                  :title="r.manuallyAdded ? '取消标记' : '加入错题本'"
                  @click.stop="r.manuallyAdded = !r.manuallyAdded"
                >
                  <Icon
                    :name="r.manuallyAdded ? 'BookmarkCheck' : 'Bookmark'"
                    :size="14"
                    :class="{ marked: r.manuallyAdded }"
                  />
                </button>
              </div>
            </li>
          </ul>

          <div class="sm-actions">
            <button class="fr-btn fr-btn-primary" @click="restartTraining">
              <Icon name="RotateCcw" :size="14" />
              <span>再来一套</span>
            </button>
          </div>
        </div>
      </section>
    </Transition>
  </div>
</template>

<style scoped>
.training-root {
  height: 100%;
  width: 100%;
  position: relative;
  overflow: hidden;
  background: var(--bg);
}
.page {
  position: absolute;
  inset: 0;
  overflow-y: auto;
}

.page-enter-active { animation: pageIn var(--dur-base) var(--ease); }
.page-leave-active { animation: pageOut var(--dur-fast) var(--ease) forwards; }
@keyframes pageIn  { from { opacity: 0; transform: translateY(12px); } to { opacity: 1; transform: none; } }
@keyframes pageOut { from { opacity: 1; } to { opacity: 0; } }

/* ── Setup ───────────────────── */
.setup {
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding: var(--sp-12) var(--sp-6);
}
.setup-card {
  width: 100%;
  max-width: 720px;
  padding: var(--sp-8);
  display: flex;
  flex-direction: column;
  gap: var(--sp-6);
}
.setup-head h1 {
  font-size: var(--fs-20);
  font-weight: var(--fw-semibold);
  color: var(--text);
  margin-bottom: 4px;
}
.setup-head p { font-size: var(--fs-13); color: var(--text-muted); }

.block { display: flex; flex-direction: column; gap: 10px; }
.block-head {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
}
.block-label {
  font-size: var(--fs-12);
  font-weight: var(--fw-medium);
  color: var(--text-muted);
}
.block-aside {
  font-size: var(--fs-12);
  color: var(--text-subtle);
}
.block-aside.ok  { color: var(--accent); }
.block-aside.off { color: var(--text-subtle); }
.block-aside strong { font-weight: var(--fw-semibold); color: var(--text); margin: 0 1px; }

.tag-grid { display: flex; flex-wrap: wrap; gap: 6px; }
.tag-pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-muted);
  font-size: var(--fs-13);
  transition: all var(--dur-fast) var(--ease);
}
.tag-pill:hover:not(.active) {
  border-color: var(--border-strong);
  color: var(--text);
}
.tag-pill.active {
  background: var(--accent-soft);
  color: var(--accent);
  border-color: transparent;
  font-weight: var(--fw-medium);
}
.tag-count {
  font-size: 11px;
  opacity: 0.7;
}

.grid-2 {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--sp-4);
}

.toggle {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border);
  background: var(--surface);
  font-size: var(--fs-13);
  color: var(--text-muted);
  transition: all var(--dur-fast) var(--ease);
}
.toggle:hover { border-color: var(--border-strong); }
.toggle.on {
  border-color: var(--accent);
  color: var(--accent);
  background: var(--accent-soft);
}
.toggle-track {
  width: 28px;
  height: 16px;
  border-radius: 999px;
  background: var(--border-strong);
  position: relative;
  flex-shrink: 0;
  transition: background var(--dur-fast) var(--ease);
}
.toggle.on .toggle-track { background: var(--accent); }
.toggle-thumb {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--surface);
  transition: transform var(--dur-fast) var(--ease);
}
.toggle.on .toggle-thumb { transform: translateX(12px); }

.notice {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 10px 12px;
  border-radius: var(--radius-md);
  font-size: var(--fs-13);
  line-height: 1.5;
}
.notice-danger {
  background: var(--danger-soft);
  color: var(--danger);
}
.notice-warning {
  background: var(--warning-soft);
  color: var(--warning);
}
.insufficient-tag {
  display: inline-block;
  padding: 1px 6px;
  margin: 0 2px;
  border-radius: var(--radius-sm);
  background: var(--surface);
  font-size: var(--fs-12);
  color: var(--warning);
  border: 1px solid var(--warning);
}

.start-btn {
  width: 100%;
  padding: 12px;
  font-size: var(--fs-14);
  font-weight: var(--fw-semibold);
}
.start-hint { opacity: 0.85; font-weight: var(--fw-regular); }

/* ── Interview ───────────────────── */
.interview { display: flex; flex-direction: column; }
.progress-track {
  height: 2px;
  background: var(--surface-2);
  position: sticky;
  top: 0;
  z-index: 10;
}
.progress-fill {
  height: 100%;
  background: var(--accent);
  transition: width var(--dur-base) var(--ease);
}

.exit-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px var(--sp-6);
  background: var(--warning-soft);
  border-bottom: 1px solid var(--warning);
  font-size: var(--fs-13);
  color: var(--warning);
}
.exit-actions { display: flex; gap: 8px; }
.confirm-enter-active, .confirm-leave-active { transition: all var(--dur-base) var(--ease); }
.confirm-enter-from, .confirm-leave-to { opacity: 0; transform: translateY(-6px); }

.preload {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--sp-3);
  padding: var(--sp-12);
  color: var(--text-muted);
  font-size: var(--fs-13);
}

.iv-content {
  max-width: 960px;
  margin: 0 auto;
  width: 100%;
  padding: var(--sp-6) var(--sp-6) var(--sp-12);
  display: flex;
  flex-direction: column;
  gap: var(--sp-4);
}

.iv-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.iv-index {
  font-size: var(--fs-13);
  font-weight: var(--fw-semibold);
  color: var(--text);
}
.iv-diff { color: var(--warning); }
.iv-timer {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: var(--fs-12);
  color: var(--text-muted);
  padding: 3px 8px;
  border-radius: 999px;
  background: var(--surface);
  border: 1px solid var(--border);
}
.iv-exit {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  margin-left: auto;
  padding: 4px 10px;
  border-radius: var(--radius-sm);
  font-size: var(--fs-12);
  color: var(--text-subtle);
  transition: all var(--dur-fast) var(--ease);
}
.iv-exit:hover {
  color: var(--danger);
  background: var(--danger-soft);
}

.q-card {
  padding: var(--sp-6);
}
.q-text {
  font-size: var(--fs-16);
  line-height: 1.7;
  color: var(--text);
}

.a-card {
  padding: var(--sp-6);
  display: flex;
  flex-direction: column;
  gap: var(--sp-4);
}

.options { display: flex; flex-direction: column; gap: 8px; }
.multi-hint {
  font-size: var(--fs-12);
  color: var(--text-muted);
  margin-bottom: 4px;
}
.option {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  padding: 10px var(--sp-3);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  cursor: pointer;
  transition: all var(--dur-fast) var(--ease);
}
.option:hover:not(.disabled) {
  border-color: var(--border-strong);
  background: var(--surface-2);
}
.option.selected {
  border-color: var(--accent);
  background: var(--accent-soft);
}
.option.disabled { cursor: default; opacity: 0.85; }
.option input {
  position: absolute;
  opacity: 0;
  pointer-events: none;
}
.opt-letter {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: var(--surface-2);
  border: 1px solid var(--border);
  font-size: var(--fs-12);
  font-weight: var(--fw-semibold);
  color: var(--text-muted);
  flex-shrink: 0;
}
.option.selected .opt-letter {
  background: var(--accent);
  border-color: var(--accent);
  color: var(--text-on-accent);
}
.opt-text {
  font-size: var(--fs-14);
  color: var(--text);
  line-height: 1.5;
}

.essay-input {
  width: 100%;
  min-height: 160px;
  padding: var(--sp-3);
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  color: var(--text);
  font-size: var(--fs-14);
  font-family: var(--font-sans);
  line-height: 1.6;
  resize: vertical;
  outline: none;
  transition: border-color var(--dur-fast) var(--ease), box-shadow var(--dur-fast) var(--ease);
}
.essay-input:focus {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-soft);
}
.essay-input::placeholder { color: var(--text-subtle); }

.a-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

/* Result */
.result-card {
  padding: var(--sp-6);
  display: flex;
  flex-direction: column;
  gap: var(--sp-4);
}
.result-enter-active, .result-leave-active { transition: all var(--dur-base) var(--ease); }
.result-enter-from, .result-leave-to { opacity: 0; transform: translateY(8px); }

.result-top {
  display: flex;
  align-items: center;
  gap: var(--sp-4);
}
.score-badge {
  padding: 6px 14px;
  border-radius: var(--radius-md);
  font-weight: var(--fw-semibold);
  display: inline-flex;
  align-items: baseline;
  gap: 2px;
}
.score-badge.pass { background: var(--success-soft); color: var(--success); }
.score-badge.fail { background: var(--danger-soft);  color: var(--danger); }
.score-badge.score-na { background: var(--surface-2); color: var(--text-subtle); font-weight: var(--fw-regular); }
.score-num { font-size: var(--fs-20); }
.score-unit { font-size: var(--fs-12); font-weight: var(--fw-regular); }

.verdict {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: var(--fs-13);
  font-weight: var(--fw-medium);
}
.verdict.correct { color: var(--success); }
.verdict.wrong   { color: var(--danger); }

.result-section {
  border-top: 1px solid var(--border);
  padding-top: var(--sp-3);
}
.result-section:first-of-type { border-top: none; padding-top: 0; }
.rs-label {
  font-size: 11px;
  font-weight: var(--fw-semibold);
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin-bottom: 6px;
}
.rs-text {
  font-size: var(--fs-13);
  color: var(--text);
  line-height: 1.65;
}

.result-actions {
  display: flex;
  justify-content: space-between;
  gap: 8px;
}

/* ── Summary ───────────────────── */
.summary { padding: var(--sp-6); }
.sm-wrap {
  max-width: var(--content-max);
  margin: 0 auto;
}
.sm-head {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: var(--sp-4);
  margin-bottom: var(--sp-6);
}
.sm-head h2 {
  font-size: var(--fs-20);
  font-weight: var(--fw-semibold);
  color: var(--text);
  margin-bottom: 4px;
}
.sm-head p { font-size: var(--fs-13); color: var(--text-muted); }

.stats {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: var(--sp-4);
  margin-bottom: var(--sp-6);
}
.stat {
  padding: var(--sp-4) var(--sp-6);
}
.stat-label {
  font-size: var(--fs-12);
  color: var(--text-muted);
  margin-bottom: var(--sp-2);
}
.stat-value {
  font-size: var(--fs-28);
  font-weight: var(--fw-semibold);
  color: var(--text);
  line-height: 1.1;
}
.stat-value.good { color: var(--success); }
.stat-value.bad  { color: var(--danger); }
.stat-unit {
  font-size: var(--fs-14);
  font-weight: var(--fw-regular);
  color: var(--text-subtle);
  margin-left: 2px;
}

.result-list {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  overflow: hidden;
}
.result-row {
  display: grid;
  grid-template-columns: 56px 1fr auto;
  gap: var(--sp-4);
  padding: var(--sp-4) var(--sp-6);
  border-bottom: 1px solid var(--border);
}
.result-row:last-child { border-bottom: none; }
.result-row.correct .row-status { color: var(--success); }
.result-row.wrong   .row-status { color: var(--danger); }
.result-row.skipped .row-status { color: var(--text-subtle); }

.row-left {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
}
.row-num {
  font-size: var(--fs-12);
  color: var(--text-subtle);
}

.row-body { min-width: 0; }
.row-q {
  font-size: var(--fs-14);
  color: var(--text);
  line-height: 1.6;
  margin-bottom: 6px;
}
.row-meta {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
  margin-bottom: 6px;
}
.fr-chip.pin {
  background: var(--surface-2);
  color: var(--text-subtle);
}

.ans-inline {
  font-size: var(--fs-13);
  color: var(--text-muted);
  display: flex;
  gap: 16px;
  margin-bottom: 6px;
  flex-wrap: wrap;
}
.your-ans strong, .std-ans strong { color: var(--text); }
.std-ans strong { color: var(--success); }

.ans-blocks {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 6px;
}
.ans-block {
  background: var(--surface-2);
  border-radius: var(--radius-md);
  padding: 8px 12px;
}
.ans-label {
  font-size: 11px;
  font-weight: var(--fw-medium);
  color: var(--text-muted);
  margin-bottom: 2px;
}
.ans-block p {
  font-size: var(--fs-13);
  color: var(--text);
  line-height: 1.6;
}

.row-comment {
  font-size: var(--fs-12);
  color: var(--text-muted);
  line-height: 1.6;
  margin-top: 6px;
}

.row-right {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 4px;
  flex-shrink: 0;
}
.row-score {
  font-size: var(--fs-20);
  font-weight: var(--fw-semibold);
  line-height: 1;
}
.row-score.good { color: var(--success); }
.row-score.bad  { color: var(--danger); }
.row-time {
  font-size: 11px;
  color: var(--text-subtle);
}
.row-mark {
  width: 24px;
  height: 24px;
  border-radius: var(--radius-sm);
  color: var(--text-subtle);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all var(--dur-fast) var(--ease);
}
.row-mark:hover { color: var(--accent); background: var(--accent-soft); }
.row-mark .marked { color: var(--accent); }

.sm-actions {
  display: flex;
  justify-content: center;
  margin-top: var(--sp-6);
}

.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
</style>
