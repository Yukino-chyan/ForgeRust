<script setup lang="ts">
import { ref, computed, inject, onMounted, onUnmounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";

interface AiResponse {
  standard_answer: string
  explanation: string
  is_correct: boolean | null
  ai_comment: string
  score: number
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

const props = defineProps<{ wrongPracticeIds?: number[] }>();
const emit = defineEmits<{ consumed: [] }>();

// 错题本触发练习：收到 ids 后自动组卷
watch(() => props.wrongPracticeIds, async (ids) => {
  if (!ids || ids.length === 0) return;
  try {
    questionList.value = await invoke<Question[]>("generate_interview_from_ids", { questionIds: ids });
    currentIndex.value = 0;
    userAnswer.value = "";
    multiAnswers.value = [];
    aiResult.value = null;
    trainingResults.value = [];
    showExitConfirm.value = false;
    appState.value = 'interview';
    startTimer();
    emit('consumed');
  } catch (e) { alert(e); }
}, { immediate: true });

const tags = ref<string[]>([]);
const tagCounts = ref<Record<string, number>>({});
const injectedApiKey = inject<ReturnType<typeof ref<string>>>("apiKey");
const hasApiKey = computed(() => !!injectedApiKey?.value?.trim());

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

const appState = ref<'setup' | 'interview' | 'summary'>('setup');
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

// ── 退出确认 ──
const showExitConfirm = ref(false);

// ── 计时器 ──
const elapsedSeconds = ref(0);
let timerInterval: ReturnType<typeof setInterval> | null = null;

const formattedTime = computed(() => {
  const m = Math.floor(elapsedSeconds.value / 60);
  const s = elapsedSeconds.value % 60;
  return `${m}:${s.toString().padStart(2, '0')}`;
});

function startTimer() {
  elapsedSeconds.value = 0;
  if (timerInterval) clearInterval(timerInterval);
  timerInterval = setInterval(() => { elapsedSeconds.value++; }, 1000);
}

function stopTimer(): number {
  if (timerInterval) { clearInterval(timerInterval); timerInterval = null; }
  return elapsedSeconds.value;
}

onUnmounted(() => { if (timerInterval) clearInterval(timerInterval); });

const canSubmit = computed(() => {
  if (!currentQuestion.value) return false;
  if (currentQuestion.value.question_type === 'MULTI') return multiAnswers.value.length > 0;
  return !!userAnswer.value;
});

// 考虑实际库存后的真实预期题数
const expectedCount = computed(() =>
  selectedTags.value.reduce((sum, tag) =>
    sum + Math.min(countPerTag.value, tagCounts.value[tag] ?? 0), 0)
);

// 库存不足的考点
const insufficientTags = computed(() =>
  selectedTags.value.filter(tag => (tagCounts.value[tag] ?? 0) < countPerTag.value)
);

const currentQuestion = computed(() => questionList.value[currentIndex.value]);
const currentOptions = computed(() => {
  if (!currentQuestion.value || !currentQuestion.value.options) return [];
  try { return JSON.parse(currentQuestion.value.options); }
  catch (e) { return []; }
});
const progress = computed(() =>
  questionList.value.length ? Math.round(((currentIndex.value) / questionList.value.length) * 100) : 0
);

const totalCount   = computed(() => trainingResults.value.length);
const skippedCount = computed(() => trainingResults.value.filter(r => r.skipped).length);
const attemptedResults = computed(() => trainingResults.value); // 跳过题算0分，全部参与统计
const correctCount = computed(() =>
  attemptedResults.value.filter(r => {
    if (r.evaluation.score === -1) return false;
    return r.evaluation.is_correct !== null ? r.evaluation.is_correct : r.evaluation.score >= 60;
  }).length
);
const scoredResults = computed(() => attemptedResults.value.filter(r => r.evaluation.score >= 0));
const averageScore  = computed(() => {
  if (scoredResults.value.length === 0) return '--';
  return Math.round(scoredResults.value.reduce((a, r) => a + r.evaluation.score, 0) / scoredResults.value.length);
});
const accuracyRate  = computed(() =>
  totalCount.value === 0 ? 0 : Math.round((correctCount.value / totalCount.value) * 100)
);

const difficultyLabel = (d: number) => ['', '入门', '简单', '中等', '困难', '专家'][d] ?? '未知';
const typeLabel = (t: string) => ({ SINGLE: '单选', MULTI: '多选', ESSAY: '简答' }[t] ?? t);

// 兼容 "A. xxx" / "A、xxx" / "A) xxx" / "(A) xxx" 等多种格式
function parseOption(opt: string): { letter: string; text: string } {
  const match = opt.match(/^[（(]?([A-Za-z])[)）.、。：:\s]+(.+)$/s);
  if (match) return { letter: match[1].toUpperCase(), text: match[2].trim() };
  return { letter: opt.charAt(0).toUpperCase(), text: opt.slice(1).trimStart() };
}

// 当前题对应的答题结果（提交后才有）
const currentResult = computed(() =>
  trainingResults.value.find(r => r.question.id === currentQuestion.value?.id)
);

function toggleManualMark() {
  const r = currentResult.value;
  if (r) r.manuallyAdded = !r.manuallyAdded;
}

async function saveTrainingSession() {
  if (trainingResults.value.length === 0) return;
  try {
    await invoke("save_training_session", {
      records: trainingResults.value.map(r => ({
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
  if (selectedTags.value.length === 0) return alert("请至少选择一个考点！");
  try {
    questionList.value = await invoke("generate_interview", { tags: selectedTags.value, count: countPerTag.value });
    currentIndex.value = 0;
    userAnswer.value = "";
    multiAnswers.value = [];
    aiResult.value = null;
    trainingResults.value = [];
    showExitConfirm.value = false;
    appState.value = 'interview';
    startTimer();
  } catch (error) { alert(error); }
}

async function startEvaluation() {
  if (currentQuestion.value?.question_type === 'MULTI') {
    userAnswer.value = [...multiAnswers.value].sort().join(',');
  }
  if (!userAnswer.value) return alert("请先给出你的答案！");

  if (!useAI.value && currentQuestion.value?.question_type === 'ESSAY') {
    const q = currentQuestion.value;
    const result: AiResponse = {
      standard_answer: q.standard_answer,
      explanation: q.explanation,
      is_correct: null,
      ai_comment: "AI 点评已关闭，请对照标准答案自行评估。",
      score: -1,
    };
    aiResult.value = result;
    trainingResults.value.push({ question: q, userAnswer: userAnswer.value, evaluation: result, timeSpent: stopTimer(), skipped: false, manuallyAdded: false });
    return;
  }

  isLoading.value = true;
  try {
    const result: AiResponse = await invoke("evaluate_answer", {
      questionId: currentQuestion.value.id,
      userAnswer: userAnswer.value
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
    userAnswer: '',
    evaluation: {
      standard_answer: q.standard_answer,
      explanation: q.explanation,
      is_correct: false,
      ai_comment: '',
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
    appState.value = 'summary';
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
    appState.value = 'summary';
    saveTrainingSession();
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
  stopTimer();
  appState.value = 'setup';
  selectedTags.value = [];
  trainingResults.value = [];
  userAnswer.value = "";
  multiAnswers.value = [];
  aiResult.value = null;
  showExitConfirm.value = false;
}
</script>

<template>
  <div class="training-container">

    <!-- ===== Setup 页 ===== -->
    <Transition name="page">
      <div v-if="appState === 'setup'" class="page setup-page" key="setup">
        <div class="setup-card">
          <div class="setup-header">
            <h1 class="setup-title">题库专项训练</h1>
            <p class="setup-subtitle">选择考点，AI 将为你生成专项练习并实时点评</p>
          </div>

          <div class="tag-section">
            <div class="section-label">
              <span>选择考点</span>
              <span v-if="selectedTags.length" class="tag-count">已选 {{ selectedTags.length }} 个</span>
            </div>
            <div class="tag-grid">
              <button
                v-for="tag in tags" :key="tag"
                :class="['tag-btn', { selected: selectedTags.includes(tag) }]"
                @click="toggleTag(tag)"
              >
                <span>{{ tag }}</span>
                <span class="tag-count-badge">{{ tagCounts[tag] ?? 0 }}</span>
              </button>
            </div>
          </div>

          <div class="bottom-options">
            <div class="count-section">
              <div class="section-label">
                <span>每个考点题数</span>
                <span class="tag-count">实际约 {{ expectedCount }} 道题</span>
              </div>
              <input
                type="number"
                class="count-input"
                v-model.number="countPerTag"
                min="1"
                max="20"
                placeholder="输入题目数量"
              />
            </div>

            <div class="ai-section">
              <div class="section-label">
                <span>AI 点评</span>
                <span :class="['ai-status', useAI ? 'on' : 'off']">{{ useAI ? '已开启' : '已关闭' }}</span>
              </div>
              <button :class="['ai-toggle', { active: useAI }]" @click="useAI = !useAI">
                <span class="toggle-track">
                  <span class="toggle-thumb"></span>
                </span>
                <span class="toggle-label">{{ useAI ? '简答题将由 AI 评分' : '仅展示标准答案' }}</span>
              </button>
            </div>
          </div>

          <div v-if="useAI && !hasApiKey" class="api-warning">
            ⚠️ 已开启 AI 点评，但尚未配置 API Key。请点击左侧「API 设置」填写后再开始。
          </div>

          <div v-if="insufficientTags.length > 0" class="insufficient-warning">
            ⚠️ 以下考点题目数量不足，将按实际库存出题：
            <span v-for="tag in insufficientTags" :key="tag" class="insufficient-tag">
              {{ tag }}（共 {{ tagCounts[tag] }} 题）
            </span>
          </div>

          <button
            class="start-btn"
            @click="startInterview"
            :disabled="selectedTags.length === 0 || (useAI && !hasApiKey) || expectedCount === 0"
          >
            <span>开始训练</span>
            <span v-if="selectedTags.length" class="start-hint">{{ expectedCount }} 道题</span>
          </button>
        </div>
      </div>
    </Transition>

    <!-- ===== 答题页 ===== -->
    <Transition name="page">
      <div v-if="appState === 'interview'" class="page interview-page" key="interview">

        <!-- 顶部进度条 -->
        <div class="progress-bar-wrap">
          <div class="progress-bar" :style="{ width: progress + '%' }"></div>
        </div>

        <!-- 退出确认条 -->
        <Transition name="exit-confirm">
          <div v-if="showExitConfirm" class="exit-confirm-bar">
            <span>确定退出本次训练？已答记录将丢失。</span>
            <div class="exit-confirm-actions">
              <button class="exit-cancel-btn" @click="showExitConfirm = false">继续训练</button>
              <button class="exit-ok-btn" @click="confirmExit">确定退出</button>
            </div>
          </div>
        </Transition>

        <div class="interview-content">
          <!-- 题号 & 元信息 -->
          <div class="question-meta">
            <span class="q-index">{{ currentIndex + 1 }} / {{ questionList.length }}</span>
            <span class="q-tag">{{ currentQuestion?.tags }}</span>
            <span class="q-type">{{ typeLabel(currentQuestion?.question_type) }}</span>
            <span :class="['q-diff', `diff-${currentQuestion?.difficulty}`]">
              {{ difficultyLabel(currentQuestion?.difficulty) }}
            </span>
            <span class="q-timer">⏱ {{ formattedTime }}</span>
            <button class="exit-btn" @click="exitTraining">退出</button>
          </div>

          <!-- 题目内容 -->
          <div class="question-card">
            <p class="question-text">{{ currentQuestion?.content }}</p>
          </div>

          <!-- 答题区 -->
          <div class="answer-card">
            <!-- 单选 -->
            <div v-if="currentQuestion?.question_type === 'SINGLE'" class="options-list">
              <label
                v-for="opt in currentOptions" :key="opt"
                :class="['option-item', {
                  'selected': userAnswer === parseOption(opt).letter,
                  'disabled': !!aiResult
                }]"
              >
                <input type="radio" name="single" :value="parseOption(opt).letter" v-model="userAnswer"
                  :disabled="isLoading || !!aiResult" />
                <span class="opt-letter">{{ parseOption(opt).letter }}</span>
                <span class="opt-text">{{ parseOption(opt).text }}</span>
              </label>
            </div>

            <!-- 多选 -->
            <div v-else-if="currentQuestion?.question_type === 'MULTI'" class="options-list">
              <p class="multi-hint">多选题 · 请选择所有正确选项</p>
              <label
                v-for="opt in currentOptions" :key="opt"
                :class="['option-item', {
                  'selected': multiAnswers.includes(parseOption(opt).letter),
                  'disabled': !!aiResult
                }]"
              >
                <input type="checkbox" :value="parseOption(opt).letter" v-model="multiAnswers"
                  :disabled="isLoading || !!aiResult" />
                <span class="opt-letter">{{ parseOption(opt).letter }}</span>
                <span class="opt-text">{{ parseOption(opt).text }}</span>
              </label>
            </div>

            <!-- 简答 -->
            <div v-else-if="currentQuestion?.question_type === 'ESSAY'">
              <textarea
                v-model="userAnswer"
                placeholder="请详细阐述你的回答..."
                :disabled="isLoading || !!aiResult"
              ></textarea>
            </div>

            <div class="answer-actions">
              <button
                class="skip-btn"
                @click="skipQuestion"
                :disabled="isLoading || !!aiResult"
              >跳过此题</button>
              <button
                class="submit-btn"
                @click="startEvaluation"
                :disabled="isLoading || !canSubmit || !!aiResult"
              >
                <span v-if="isLoading" class="loading-dots">批阅中<span>.</span><span>.</span><span>.</span></span>
                <span v-else>提交回答</span>
              </button>
            </div>
          </div>

          <!-- 评阅结果 -->
          <Transition name="result">
            <div v-if="aiResult" class="result-card">
              <div class="result-score-row">
                <div v-if="aiResult.score !== -1" :class="['score-badge', aiResult.score >= 60 ? 'pass' : 'fail']">
                  {{ aiResult.score }}
                  <span class="score-unit">分</span>
                </div>
                <div v-else class="score-badge no-score">未评分</div>
                <div v-if="aiResult.is_correct !== null" :class="['verdict', aiResult.is_correct ? 'correct' : 'wrong']">
                  {{ aiResult.is_correct ? '✅ 回答正确' : '❌ 回答错误' }}
                </div>
              </div>

              <div class="result-section ai-comment">
                <div class="result-section-label">AI 点评</div>
                <p>{{ aiResult.ai_comment }}</p>
              </div>

              <div class="result-section std-answer">
                <div class="result-section-label">📌 标准答案</div>
                <p>{{ aiResult.standard_answer }}</p>
              </div>

              <div class="result-section explanation">
                <div class="result-section-label">📖 解析</div>
                <p>{{ aiResult.explanation }}</p>
              </div>

              <div class="result-actions">
                <button
                  :class="['mark-btn', { marked: currentResult?.manuallyAdded }]"
                  @click="toggleManualMark"
                >
                  {{ currentResult?.manuallyAdded ? '✅ 已加入错题本' : '📌 加入错题本' }}
                </button>
                <button class="next-btn" @click="nextQuestion">
                  {{ currentIndex < questionList.length - 1 ? '下一题 →' : '查看报告 →' }}
                </button>
              </div>
            </div>
          </Transition>
        </div>
      </div>
    </Transition>

    <!-- ===== 总结页 ===== -->
    <Transition name="page">
      <div v-if="appState === 'summary'" class="page summary-page" key="summary">
        <div class="summary-content">
          <div class="summary-header">
            <h2>训练完成</h2>
            <p>本次共 {{ totalCount }} 道题，以下是你的表现</p>
          </div>

          <div class="stat-cards">
            <div class="stat-card">
              <div :class="['stat-value', accuracyRate >= 60 ? 'good' : 'bad']">{{ accuracyRate }}<span class="stat-unit">%</span></div>
              <div class="stat-label">正确率</div>
            </div>
            <div class="stat-card">
              <div class="stat-value neutral">{{ correctCount }}<span class="stat-unit">/{{ totalCount }}</span></div>
              <div class="stat-label">答对题数</div>
            </div>
            <div class="stat-card">
              <div :class="['stat-value', averageScore === '--' ? 'neutral' : (averageScore as number) >= 60 ? 'good' : 'bad']">{{ averageScore }}</div>
              <div class="stat-label">平均分</div>
            </div>
            <div class="stat-card">
              <div :class="['stat-value', skippedCount > 0 ? 'bad' : 'good']">{{ skippedCount }}</div>
              <div class="stat-label">跳过题数</div>
            </div>
          </div>

          <div class="result-list">
            <div
              v-for="(r, idx) in trainingResults" :key="idx"
              :class="['result-item',
                r.skipped ? 'item-skipped' :
                (r.evaluation.is_correct === true || (r.evaluation.is_correct === null && r.evaluation.score >= 60))
                  ? 'item-correct' : 'item-wrong'
              ]"
            >
              <div class="item-left">
                <span class="item-num">{{ idx + 1 }}</span>
                <span class="item-status">{{ r.skipped ? '⏭' : (r.evaluation.is_correct === true || (r.evaluation.is_correct === null && r.evaluation.score >= 60)) ? '✅' : '❌' }}</span>
              </div>

              <div class="item-body">
                <p class="item-question">{{ r.question.content }}</p>
                <div class="item-tags">
                  <span class="itag">{{ r.question.tags }}</span>
                  <span class="itag">{{ typeLabel(r.question.question_type) }}</span>
                  <span v-if="r.skipped" class="itag itag-skipped">已跳过</span>
                </div>

                <!-- 跳过：只展示标准答案供参考 -->
                <div v-if="r.skipped" class="item-answers-essay">
                  <div class="essay-block std-ans-block">
                    <span class="essay-label">标准答案（供参考）</span>
                    <p>{{ r.question.standard_answer }}</p>
                  </div>
                </div>

                <!-- 选择题：单行展示 -->
                <div v-else-if="r.evaluation.is_correct !== null" class="item-answers">
                  <span class="your-ans">你的答案：{{ r.userAnswer }}</span>
                  <span v-if="r.evaluation.is_correct === false" class="std-ans">
                    正确答案：{{ r.evaluation.standard_answer }}
                  </span>
                </div>

                <!-- 简答题：块级展示，始终显示标准答案 -->
                <div v-else class="item-answers-essay">
                  <div class="essay-block your-ans-block">
                    <span class="essay-label">你的回答</span>
                    <p>{{ r.userAnswer }}</p>
                  </div>
                  <div class="essay-block std-ans-block">
                    <span class="essay-label">标准答案</span>
                    <p>{{ r.evaluation.standard_answer }}</p>
                  </div>
                </div>

                <p class="item-comment">{{ r.evaluation.ai_comment }}</p>
              </div>

              <div class="item-score">
                <span :class="r.evaluation.score >= 60 ? 'score-good' : 'score-bad'">{{ r.evaluation.score }}</span>
                <span class="score-unit-sm">分</span>
                <span class="item-time">{{ Math.floor(r.timeSpent / 60) }}:{{ String(r.timeSpent % 60).padStart(2, '0') }}</span>
              </div>
            </div>
          </div>

          <button class="restart-btn" @click="restartTraining">再来一套</button>
        </div>
      </div>
    </Transition>

  </div>
</template>

<style scoped>
/* ===== 容器 ===== */
.training-container {
  height: 100%;
  overflow: hidden;
  background: #080d18;
  position: relative;
}
.page {
  position: absolute;
  inset: 0;
  overflow-y: auto;
}

/* ===== 页面切换动画 ===== */
.page-enter-active { animation: pageIn 0.35s cubic-bezier(0.4, 0, 0.2, 1); }
.page-leave-active { animation: pageOut 0.25s cubic-bezier(0.4, 0, 0.2, 1) forwards; }
@keyframes pageIn  { from { opacity: 0; transform: translateY(18px); } to { opacity: 1; transform: none; } }
@keyframes pageOut { from { opacity: 1; } to { opacity: 0; } }

/* ===== Setup 页 ===== */
.setup-page {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 40px 24px;
}
.setup-card {
  width: 100%;
  max-width: 720px;
  background: rgba(13,21,41,0.8);
  border: 1px solid rgba(99,179,237,0.12);
  border-radius: 20px;
  padding: 40px;
  backdrop-filter: blur(12px);
  box-shadow: 0 20px 60px rgba(0,0,0,0.4);
  animation: fadeInUp 0.4s ease;
}
.setup-header { text-align: center; margin-bottom: 36px; }
.setup-title {
  font-size: 1.9rem;
  font-weight: 700;
  background: linear-gradient(90deg, #4facfe, #00d4ff);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  margin-bottom: 8px;
}
.setup-subtitle { color: #4a5568; font-size: 0.9rem; }

.tag-section { margin-bottom: 32px; }
.section-label {
  display: flex;
  justify-content: space-between;
  font-size: 0.82rem;
  color: #718096;
  margin-bottom: 14px;
  font-weight: 500;
  letter-spacing: 0.05em;
  text-transform: uppercase;
}
.tag-count { color: #4facfe; }
.tag-grid { display: flex; flex-wrap: wrap; gap: 8px; }
.tag-btn {
  padding: 8px 16px;
  border-radius: 8px;
  border: 1px solid rgba(99,179,237,0.18);
  background: rgba(255,255,255,0.03);
  color: #718096;
  font-size: 0.875rem;
  cursor: pointer;
  transition: all 0.18s ease;
}
.tag-btn { display: inline-flex; align-items: center; gap: 6px; }
.tag-btn:hover { border-color: rgba(79,172,254,0.45); color: #90cdf4; background: rgba(79,172,254,0.07); }
.tag-count-badge {
  font-size: 0.68rem;
  padding: 1px 6px;
  border-radius: 10px;
  background: rgba(99,179,237,0.12);
  color: #4a5568;
  font-weight: 600;
  min-width: 18px;
  text-align: center;
}
.tag-btn.selected .tag-count-badge {
  background: rgba(79,172,254,0.2);
  color: #4facfe;
}
.tag-btn.selected {
  background: linear-gradient(135deg, rgba(79,172,254,0.2), rgba(0,212,255,0.12));
  border-color: rgba(79,172,254,0.5);
  color: #4facfe;
  font-weight: 600;
  box-shadow: 0 0 12px rgba(79,172,254,0.15);
}

.bottom-options {
  display: flex;
  gap: 16px;
}
.count-section, .ai-section {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.ai-status { font-weight: 600; }
.ai-status.on  { color: #4facfe; }
.ai-status.off { color: #4a5568; }

.ai-toggle {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 9px 14px;
  background: rgba(255,255,255,0.04);
  border: 1px solid rgba(99,179,237,0.18);
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s ease;
  text-align: left;
}
.ai-toggle:hover { border-color: rgba(79,172,254,0.35); background: rgba(79,172,254,0.05); }
.ai-toggle.active { border-color: rgba(79,172,254,0.4); background: rgba(79,172,254,0.08); }

.toggle-track {
  width: 36px;
  height: 20px;
  border-radius: 10px;
  background: rgba(255,255,255,0.1);
  display: flex;
  align-items: center;
  padding: 2px;
  flex-shrink: 0;
  transition: background 0.2s ease;
  position: relative;
}
.ai-toggle.active .toggle-track { background: linear-gradient(90deg, #4facfe, #00d4ff); }

.toggle-thumb {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 1px 4px rgba(0,0,0,0.3);
  transition: transform 0.2s ease;
}
.ai-toggle.active .toggle-thumb { transform: translateX(16px); }

.toggle-label { font-size: 0.82rem; color: #718096; }
.ai-toggle.active .toggle-label { color: #90cdf4; }

.score-badge.no-score { color: #4a5568; font-size: 1.4rem; }
.count-input {
  width: 100%;
  padding: 10px 14px;
  background: rgba(255,255,255,0.04);
  border: 1px solid rgba(99,179,237,0.18);
  border-radius: 8px;
  color: #e2e8f0;
  font-size: 0.9rem;
  outline: none;
  transition: border-color 0.2s, box-shadow 0.2s;
}
.count-input:focus {
  border-color: rgba(79,172,254,0.45);
  box-shadow: 0 0 0 3px rgba(79,172,254,0.08);
}
.count-input::placeholder { color: #4a5568; }
.count-input::-webkit-inner-spin-button,
.count-input::-webkit-outer-spin-button { opacity: 0.4; }

.insufficient-warning {
  padding: 10px 14px;
  border-radius: 8px;
  background: rgba(246,173,85,0.08);
  border: 1px solid rgba(246,173,85,0.25);
  color: #f6ad55;
  font-size: 0.82rem;
  line-height: 1.6;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}
.insufficient-tag {
  padding: 1px 8px;
  border-radius: 5px;
  background: rgba(246,173,85,0.15);
  font-weight: 600;
  font-size: 0.78rem;
}

.api-warning {
  padding: 10px 14px;
  border-radius: 8px;
  background: rgba(252,129,129,0.08);
  border: 1px solid rgba(252,129,129,0.25);
  color: #fc8181;
  font-size: 0.82rem;
  line-height: 1.5;
  margin-top: 4px;
}

.start-btn {
  width: 100%;
  margin-top: 8px;
  padding: 14px;
  border: none;
  border-radius: 12px;
  background: linear-gradient(135deg, #4facfe, #00d4ff);
  color: #080d18;
  font-size: 1rem;
  font-weight: 700;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  transition: all 0.2s ease;
  box-shadow: 0 4px 20px rgba(79,172,254,0.35);
}
.start-btn:hover:not(:disabled) {
  transform: translateY(-1px);
  box-shadow: 0 6px 28px rgba(79,172,254,0.5);
}
.start-btn:disabled { background: rgba(255,255,255,0.08); color: #4a5568; cursor: not-allowed; box-shadow: none; }
.start-hint { font-size: 0.82rem; opacity: 0.7; font-weight: 400; }

/* ===== 答题页 ===== */
.progress-bar-wrap {
  height: 3px;
  background: rgba(255,255,255,0.05);
  position: sticky;
  top: 0;
  z-index: 10;
}
.progress-bar {
  height: 100%;
  background: linear-gradient(90deg, #4facfe, #00d4ff);
  transition: width 0.4s ease;
  box-shadow: 0 0 8px rgba(79,172,254,0.6);
}

.interview-content {
  padding: 28px 32px 40px;
  max-width: 1040px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.question-meta {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}
.q-index { font-size: 0.8rem; color: #4a5568; font-weight: 600; }
.q-tag, .q-type {
  font-size: 0.72rem;
  padding: 3px 10px;
  border-radius: 6px;
  background: rgba(79,172,254,0.1);
  border: 1px solid rgba(79,172,254,0.18);
  color: #63b3ed;
}
.q-diff {
  font-size: 0.72rem;
  padding: 3px 10px;
  border-radius: 6px;
}
.diff-1, .diff-2 { background: rgba(104,211,145,0.1); border: 1px solid rgba(104,211,145,0.2); color: #68d391; }
.diff-3         { background: rgba(246,173,85,0.1);  border: 1px solid rgba(246,173,85,0.2);  color: #f6ad55; }
.diff-4, .diff-5 { background: rgba(252,129,129,0.1); border: 1px solid rgba(252,129,129,0.2); color: #fc8181; }

.question-card {
  background: rgba(13,21,41,0.7);
  border: 1px solid rgba(99,179,237,0.12);
  border-left: 3px solid #4facfe;
  border-radius: 14px;
  padding: 24px;
  backdrop-filter: blur(8px);
}
.question-text {
  font-size: 1.05rem;
  color: #e2e8f0;
  line-height: 1.7;
  font-weight: 500;
}

.answer-card {
  background: rgba(13,21,41,0.5);
  border: 1px solid rgba(99,179,237,0.1);
  border-radius: 14px;
  padding: 20px;
}

.options-list { display: flex; flex-direction: column; gap: 8px; margin-bottom: 16px; }
.multi-hint { font-size: 0.78rem; color: #4a5568; margin-bottom: 12px; }

.option-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  border-radius: 10px;
  border: 1px solid rgba(99,179,237,0.1);
  background: rgba(255,255,255,0.02);
  cursor: pointer;
  transition: all 0.18s ease;
  user-select: none;
}
.option-item:hover:not(.disabled) {
  border-color: rgba(79,172,254,0.35);
  background: rgba(79,172,254,0.06);
}
.option-item.selected {
  border-color: rgba(79,172,254,0.5);
  background: rgba(79,172,254,0.1);
}
.option-item.disabled { cursor: default; opacity: 0.8; }
.option-item input { display: none; }
.opt-letter {
  width: 26px;
  height: 26px;
  border-radius: 6px;
  background: rgba(99,179,237,0.1);
  border: 1px solid rgba(99,179,237,0.2);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.78rem;
  font-weight: 700;
  color: #63b3ed;
  flex-shrink: 0;
}
.option-item.selected .opt-letter {
  background: linear-gradient(135deg, #4facfe, #00d4ff);
  border-color: transparent;
  color: #080d18;
}
.opt-text { font-size: 0.9rem; color: #cbd5e0; line-height: 1.4; }

textarea {
  width: 100%;
  height: 130px;
  padding: 14px;
  background: rgba(255,255,255,0.04);
  border: 1px solid rgba(99,179,237,0.15);
  border-radius: 10px;
  color: #e2e8f0;
  font-size: 0.9rem;
  resize: vertical;
  outline: none;
  font-family: inherit;
  transition: border-color 0.2s;
  margin-bottom: 16px;
}
textarea:focus { border-color: rgba(79,172,254,0.4); box-shadow: 0 0 0 3px rgba(79,172,254,0.08); }
textarea:disabled { opacity: 0.5; }

/* ===== 退出确认条 ===== */
.exit-confirm-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 32px;
  background: rgba(252,129,129,0.1);
  border-bottom: 1px solid rgba(252,129,129,0.2);
  font-size: 0.875rem;
  color: #fc8181;
  gap: 16px;
}
.exit-confirm-actions { display: flex; gap: 8px; }
.exit-cancel-btn {
  padding: 6px 16px;
  border-radius: 7px;
  border: 1px solid rgba(99,179,237,0.25);
  background: transparent;
  color: #90cdf4;
  font-size: 0.82rem;
  cursor: pointer;
  transition: all 0.18s;
}
.exit-cancel-btn:hover { background: rgba(79,172,254,0.1); }
.exit-ok-btn {
  padding: 6px 16px;
  border-radius: 7px;
  border: none;
  background: rgba(252,129,129,0.2);
  color: #fc8181;
  font-size: 0.82rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.18s;
}
.exit-ok-btn:hover { background: rgba(252,129,129,0.35); }
.exit-confirm-enter-active, .exit-confirm-leave-active { transition: all 0.2s ease; }
.exit-confirm-enter-from, .exit-confirm-leave-to { opacity: 0; transform: translateY(-8px); }

/* ===== 计时器 & 退出按钮（题号行） ===== */
.q-timer {
  margin-left: auto;
  font-size: 0.78rem;
  color: #4a5568;
  font-variant-numeric: tabular-nums;
}
.exit-btn {
  padding: 4px 12px;
  border-radius: 6px;
  border: 1px solid rgba(252,129,129,0.25);
  background: transparent;
  color: #718096;
  font-size: 0.75rem;
  cursor: pointer;
  transition: all 0.18s;
  flex-shrink: 0;
}
.exit-btn:hover { border-color: rgba(252,129,129,0.5); color: #fc8181; background: rgba(252,129,129,0.08); }

/* ===== 答题操作行（跳过 + 提交） ===== */
.answer-actions {
  display: flex;
  gap: 10px;
}
.skip-btn {
  padding: 12px 20px;
  border: 1px solid rgba(99,179,237,0.15);
  border-radius: 10px;
  background: transparent;
  color: #4a5568;
  font-size: 0.875rem;
  cursor: pointer;
  transition: all 0.18s;
  white-space: nowrap;
}
.skip-btn:hover:not(:disabled) { border-color: rgba(99,179,237,0.35); color: #718096; }
.skip-btn:disabled { opacity: 0.35; cursor: not-allowed; }

/* ===== 每题用时（总结页） ===== */
.item-time {
  font-size: 0.68rem;
  color: #4a5568;
  margin-top: 2px;
  font-variant-numeric: tabular-nums;
}

.submit-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  flex: 1;
  padding: 12px;
  border: none;
  border-radius: 10px;
  background: linear-gradient(135deg, #4facfe, #00d4ff);
  color: #080d18;
  font-size: 0.95rem;
  font-weight: 700;
  cursor: pointer;
  transition: all 0.2s ease;
  box-shadow: 0 4px 16px rgba(79,172,254,0.3);
}
.submit-btn:hover:not(:disabled) { transform: translateY(-1px); box-shadow: 0 6px 24px rgba(79,172,254,0.45); }
.submit-btn:disabled { background: rgba(255,255,255,0.06); color: #4a5568; cursor: not-allowed; box-shadow: none; }

/* 批阅中动画 */
.loading-dots span {
  animation: blink 1.2s infinite;
  opacity: 0;
}
.loading-dots span:nth-child(2) { animation-delay: 0.2s; }
.loading-dots span:nth-child(3) { animation-delay: 0.4s; }
@keyframes blink { 0%,80%,100% { opacity: 0; } 40% { opacity: 1; } }

/* ===== 评阅结果 ===== */
.result-enter-active { animation: resultIn 0.4s cubic-bezier(0.4, 0, 0.2, 1); }
@keyframes resultIn { from { opacity: 0; transform: translateY(12px); } to { opacity: 1; transform: none; } }

.result-card {
  background: rgba(13,21,41,0.7);
  border: 1px solid rgba(99,179,237,0.12);
  border-radius: 14px;
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.result-score-row {
  display: flex;
  align-items: center;
  gap: 16px;
}
.score-badge {
  font-size: 2.2rem;
  font-weight: 800;
  line-height: 1;
  display: flex;
  align-items: baseline;
  gap: 4px;
}
.score-badge.pass { color: #68d391; text-shadow: 0 0 20px rgba(104,211,145,0.4); }
.score-badge.fail { color: #fc8181; text-shadow: 0 0 20px rgba(252,129,129,0.4); }
.score-unit { font-size: 1rem; font-weight: 400; }
.verdict { font-size: 0.9rem; font-weight: 600; }
.verdict.correct { color: #68d391; }
.verdict.wrong   { color: #fc8181; }

.result-section { display: flex; flex-direction: column; gap: 8px; }
.result-section-label {
  font-size: 0.75rem;
  font-weight: 600;
  color: #4a5568;
  text-transform: uppercase;
  letter-spacing: 0.06em;
}
.ai-comment p { color: #cbd5e0; font-size: 0.9rem; line-height: 1.65; }
.std-answer {
  background: rgba(104,211,145,0.06);
  border: 1px solid rgba(104,211,145,0.18);
  border-radius: 10px;
  padding: 14px;
}
.std-answer p { color: #9ae6b4; font-size: 0.88rem; line-height: 1.7; }
.explanation {
  background: rgba(79,172,254,0.05);
  border: 1px solid rgba(79,172,254,0.15);
  border-radius: 10px;
  padding: 14px;
}
.explanation p { color: #90cdf4; font-size: 0.88rem; line-height: 1.7; }

.result-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}
.mark-btn {
  padding: 8px 16px;
  border-radius: 8px;
  border: 1px dashed rgba(246,173,85,0.4);
  background: transparent;
  color: #f6ad55;
  font-size: 0.82rem;
  cursor: pointer;
  transition: all 0.18s;
}
.mark-btn:hover { background: rgba(246,173,85,0.08); border-color: rgba(246,173,85,0.7); }
.mark-btn.marked {
  border-style: solid;
  background: rgba(104,211,145,0.08);
  border-color: rgba(104,211,145,0.4);
  color: #68d391;
}

.next-btn {
  align-self: flex-end;
  padding: 10px 24px;
  border: 1px solid rgba(79,172,254,0.3);
  border-radius: 8px;
  background: rgba(79,172,254,0.1);
  color: #4facfe;
  font-size: 0.9rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.18s ease;
}
.next-btn:hover { background: rgba(79,172,254,0.2); box-shadow: 0 0 16px rgba(79,172,254,0.2); }

/* ===== 总结页 ===== */
.summary-page { display: flex; justify-content: center; }
.summary-content {
  width: 100%;
  max-width: 1040px;
  padding: 36px 32px 60px;
  display: flex;
  flex-direction: column;
  gap: 24px;
}
.summary-header { text-align: center; }
.summary-header h2 {
  font-size: 1.8rem;
  font-weight: 700;
  background: linear-gradient(90deg, #4facfe, #00d4ff);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  margin-bottom: 6px;
}
.summary-header p { color: #4a5568; font-size: 0.9rem; }

.stat-cards { display: flex; gap: 14px; }
.stat-card {
  flex: 1;
  background: rgba(13,21,41,0.7);
  border: 1px solid rgba(99,179,237,0.12);
  border-radius: 16px;
  padding: 24px 16px;
  text-align: center;
  backdrop-filter: blur(8px);
  animation: fadeInUp 0.4s ease both;
}
.stat-card:nth-child(2) { animation-delay: 0.08s; }
.stat-card:nth-child(3) { animation-delay: 0.16s; }
.stat-value {
  font-size: 2.4rem;
  font-weight: 800;
  line-height: 1;
  margin-bottom: 8px;
}
.stat-value.good    { color: #68d391; text-shadow: 0 0 20px rgba(104,211,145,0.3); }
.stat-value.bad     { color: #fc8181; text-shadow: 0 0 20px rgba(252,129,129,0.3); }
.stat-value.neutral { color: #4facfe; }
.stat-unit { font-size: 1rem; font-weight: 400; }
.stat-label { font-size: 0.8rem; color: #4a5568; font-weight: 500; }

.result-list { display: flex; flex-direction: column; gap: 10px; }
.result-item {
  display: flex;
  align-items: flex-start;
  gap: 14px;
  padding: 16px;
  border-radius: 12px;
  border: 1px solid transparent;
  background: rgba(13,21,41,0.5);
  border-left-width: 3px;
  animation: fadeInUp 0.3s ease both;
}
.item-correct { border-color: rgba(104,211,145,0.2); border-left-color: #68d391; }
.item-wrong   { border-color: rgba(252,129,129,0.15); border-left-color: #fc8181; }
.item-skipped { border-color: rgba(113,128,150,0.15); border-left-color: #4a5568; opacity: 0.75; }
.itag-skipped { background: rgba(113,128,150,0.15) !important; color: #718096 !important; border-color: rgba(113,128,150,0.2) !important; }

.item-left { display: flex; flex-direction: column; align-items: center; gap: 4px; min-width: 28px; }
.item-num  { font-size: 0.7rem; color: #4a5568; }
.item-status { font-size: 1rem; }

.item-body { flex: 1; min-width: 0; }
.item-question {
  font-size: 0.9rem;
  color: #e2e8f0;
  font-weight: 500;
  margin-bottom: 6px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.item-tags { display: flex; gap: 6px; margin-bottom: 6px; }
.itag {
  font-size: 0.68rem;
  padding: 2px 8px;
  border-radius: 4px;
  background: rgba(99,179,237,0.08);
  color: #4a5568;
  border: 1px solid rgba(99,179,237,0.12);
}
.item-answers { display: flex; gap: 12px; flex-wrap: wrap; margin-bottom: 4px; }
.your-ans { font-size: 0.8rem; color: #718096; }
.std-ans  { font-size: 0.8rem; color: #fc8181; font-weight: 600; }

.item-answers-essay { display: flex; flex-direction: column; gap: 6px; margin-bottom: 6px; }
.essay-block { border-radius: 6px; padding: 8px 10px; }
.essay-label { font-size: 0.68rem; font-weight: 600; letter-spacing: 0.04em; text-transform: uppercase; display: block; margin-bottom: 3px; }
.your-ans-block { background: rgba(255,255,255,0.03); border: 1px solid rgba(99,179,237,0.1); }
.your-ans-block .essay-label { color: #4a5568; }
.your-ans-block p { font-size: 0.8rem; color: #718096; line-height: 1.5; }
.std-ans-block { background: rgba(104,211,145,0.05); border: 1px solid rgba(104,211,145,0.15); }
.std-ans-block .essay-label { color: #68d391; }
.std-ans-block p { font-size: 0.8rem; color: #9ae6b4; line-height: 1.5; }
.item-comment { font-size: 0.8rem; color: #4a5568; line-height: 1.5; }

.item-score {
  display: flex;
  flex-direction: column;
  align-items: center;
  min-width: 44px;
}
.score-good { font-size: 1.5rem; font-weight: 800; color: #68d391; }
.score-bad  { font-size: 1.5rem; font-weight: 800; color: #fc8181; }
.score-unit-sm { font-size: 0.7rem; color: #4a5568; }

.restart-btn {
  width: 100%;
  padding: 14px;
  border: none;
  border-radius: 12px;
  background: linear-gradient(135deg, #4facfe, #00d4ff);
  color: #080d18;
  font-size: 1rem;
  font-weight: 700;
  cursor: pointer;
  transition: all 0.2s ease;
  box-shadow: 0 4px 20px rgba(79,172,254,0.3);
}
.restart-btn:hover { transform: translateY(-1px); box-shadow: 0 6px 28px rgba(79,172,254,0.45); }

@keyframes fadeInUp {
  from { opacity: 0; transform: translateY(16px); }
  to   { opacity: 1; transform: none; }
}
</style>
