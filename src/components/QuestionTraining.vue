<script setup lang="ts">
import { ref, computed , onMounted} from "vue";
import { invoke } from "@tauri-apps/api/core";

// --- 接口定义 ---
interface AiResponse {
  standard_answer: string
  explanation: string
  is_correct: boolean | null  // 选择题有值，简答题为 null
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
}
interface TrainingResult {
  question: Question;
  userAnswer: string;
  evaluation: AiResponse;
}

// --- 状态变量 ---
const tags = ref<string[]>([]);
onMounted(async () => {
  try {
    tags.value = await invoke("get_all_tags");
  } catch (e) {
    console.error("加载标签失败", e);
  }
});
const appState = ref<'setup' | 'interview' | 'summary'>('setup');
const selectedTags = ref<string[]>([]);
const questionList = ref<Question[]>([]);
const currentIndex = ref(0);

const userAnswer = ref("");
const multiAnswers = ref<string[]>([]);
const aiResult = ref<AiResponse | null>(null);
const isLoading = ref(false);
const trainingResults = ref<TrainingResult[]>([]);

const canSubmit = computed(() => {
  if (!currentQuestion.value) return false;
  if (currentQuestion.value.question_type === 'MULTI') return multiAnswers.value.length > 0;
  return !!userAnswer.value;
});

const currentQuestion = computed(() => questionList.value[currentIndex.value]);
const currentOptions = computed(() => {
  if (!currentQuestion.value || !currentQuestion.value.options) return [];
  try {
    return JSON.parse(currentQuestion.value.options);
  } catch (e) {
    return [];
  }
});

// --- 总结页统计数据 ---
const totalCount = computed(() => trainingResults.value.length);
const correctCount = computed(() =>
  trainingResults.value.filter(r => {
    if (r.evaluation.is_correct !== null) return r.evaluation.is_correct;
    return r.evaluation.score >= 60;
  }).length
);
const averageScore = computed(() => {
  if (totalCount.value === 0) return 0;
  const sum = trainingResults.value.reduce((acc, r) => acc + r.evaluation.score, 0);
  return Math.round(sum / totalCount.value);
});
const accuracyRate = computed(() =>
  totalCount.value === 0 ? 0 : Math.round((correctCount.value / totalCount.value) * 100)
);

// --- 核心逻辑 ---
function toggleTag(tag: string) {
  const index = selectedTags.value.indexOf(tag);
  if (index > -1) { selectedTags.value.splice(index, 1); }
  else { selectedTags.value.push(tag); }
}

async function startInterview() {
  if (selectedTags.value.length === 0) return alert("请至少选择一个考点！");
  try {
    questionList.value = await invoke("generate_interview", { tags: selectedTags.value });
    currentIndex.value = 0;
    userAnswer.value = "";
    multiAnswers.value = [];
    aiResult.value = null;
    trainingResults.value = [];
    appState.value = 'interview';
  } catch (error) { alert(error); }
}

async function startEvaluation() {
  if (currentQuestion.value?.question_type === 'MULTI') {
    userAnswer.value = [...multiAnswers.value].sort().join(',');
  }
  if (!userAnswer.value) return alert("请先给出你的答案！");
  isLoading.value = true;
  try {
    const result: AiResponse = await invoke("evaluate_answer", {
      questionId: currentQuestion.value.id,
      userAnswer: userAnswer.value
    });
    aiResult.value = result;
    // 记录本题结果
    trainingResults.value.push({
      question: currentQuestion.value,
      userAnswer: userAnswer.value,
      evaluation: result,
    });
  } catch (error) {
    alert(`系统内部调用报错: ${error}`);
    console.error("调用失败:", error);
  } finally {
    isLoading.value = false;
  }
}

function nextQuestion() {
  if (currentIndex.value < questionList.value.length - 1) {
    currentIndex.value++;
    userAnswer.value = "";
    multiAnswers.value = [];
    aiResult.value = null;
  } else {
    appState.value = 'summary';
  }
}

function restartTraining() {
  appState.value = 'setup';
  selectedTags.value = [];
  trainingResults.value = [];
  userAnswer.value = "";
  multiAnswers.value = [];
}
</script>

<template>
  <div class="training-container">
    <div v-if="appState === 'setup'" class="setup-room">
      <div class="setup-content">
        <div class="setup-title-group">
          <h1 class="setup-title">题库专项训练</h1>
          <p class="setup-subtitle">选择你要复习的考点，系统将为你生成专项练习题</p>
        </div>

        <div class="setup-panel">
          <div class="panel-header">
            <span class="panel-label">选择考点</span>
            <span class="selected-hint" v-if="selectedTags.length > 0">
              已选 {{ selectedTags.length }} 个
            </span>
          </div>
          <div class="tag-grid">
            <button
              v-for="tag in tags" :key="tag" @click="toggleTag(tag)"
              :class="['tag-btn', { 'selected': selectedTags.includes(tag) }]"
            >
              {{ tag }}
            </button>
          </div>
        </div>

        <button class="start-btn" @click="startInterview" :disabled="selectedTags.length === 0">
          开始训练
          <span v-if="selectedTags.length > 0" class="start-hint">· 约 {{ selectedTags.length * 2 }} 道题</span>
        </button>
      </div>
    </div>

    <div v-else-if="appState === 'interview'" class="interview-room">
      <section class="question-box">
        <h3>🎙️ 考题 ({{ currentIndex + 1 }} / {{ questionList.length }})：</h3>
        <p class="question-text">{{ currentQuestion?.content }}</p>
        <span class="meta">标签：{{ currentQuestion?.tags }} | 难度：{{ currentQuestion?.difficulty }}</span>
      </section>

      <section class="answer-section">
        
        <div v-if="currentQuestion?.question_type === 'SINGLE'" class="options-container">
          <label v-for="opt in currentOptions" :key="opt" class="option-label">
            <input
              type="radio"
              name="single_choice"
              :value="opt.charAt(0)"
              v-model="userAnswer"
              :disabled="isLoading || !!aiResult"
            >
            {{ opt }}
          </label>
        </div>

        <div v-else-if="currentQuestion?.question_type === 'MULTI'" class="options-container">
          <p class="multi-hint">多选题，请选择所有正确答案</p>
          <label v-for="opt in currentOptions" :key="opt" class="option-label">
            <input
              type="checkbox"
              :value="opt.charAt(0)"
              v-model="multiAnswers"
              :disabled="isLoading || !!aiResult"
            >
            {{ opt }}
          </label>
        </div>

        <div v-else-if="currentQuestion?.question_type === 'ESSAY'">
          <textarea
            v-model="userAnswer"
            placeholder="请详细阐述你的回答..."
            :disabled="isLoading || !!aiResult"
          ></textarea>
        </div>

        <br />
        <button class="submit-btn" @click="startEvaluation" :disabled="isLoading || !canSubmit">
          {{ isLoading ? "批阅中..." : "提交回答" }}
        </button>
      </section>
  
      <!-- ✅ 新的 -->  
      <section v-if="aiResult" class="result-box">  
        <hr />  
        <h3>📈 评价反馈</h3>  
        
        <!-- 得分 -->  
        <p>  
          <strong>得分：</strong>   
          <span class="score">{{ aiResult.score }}</span>  
        </p>  
        
        <!-- 选择题：对错标识 -->  
        <p v-if="aiResult.is_correct !== null">  
          <strong>结果：</strong>  
          <span :class="aiResult.is_correct ? 'correct' : 'wrong'">  
            {{ aiResult.is_correct ? '✅ 回答正确' : '❌ 回答错误' }}  
          </span>  
        </p>  
        
        <!-- AI 点评 -->  
        <p><strong>点评：</strong> {{ aiResult.ai_comment }}</p>  
        
        <!-- 标准答案 -->  
        <div class="answer-box">  
          <strong>📌 标准答案：</strong>  
          <p>{{ aiResult.standard_answer }}</p>  
        </div>  
        
        <!-- 题目解析 -->  
        <div class="explanation-box">  
          <strong>📖 解析：</strong>  
          <p>{{ aiResult.explanation }}</p>  
        </div>  
        
        <button class="next-btn" @click="nextQuestion">👉 下一题</button>
      </section>
    </div>

    <!-- ===== 总结页 ===== -->
    <div v-else-if="appState === 'summary'" class="summary-room">
      <div class="summary-header">
        <h2>训练完成</h2>
        <p class="summary-subtitle">本次共 {{ totalCount }} 道题，以下是你的表现</p>
      </div>

      <!-- 三个统计卡片 -->
      <div class="stat-cards">
        <div class="stat-card">
          <span class="stat-value" :class="accuracyRate >= 60 ? 'good' : 'bad'">{{ accuracyRate }}%</span>
          <span class="stat-label">正确率</span>
        </div>
        <div class="stat-card">
          <span class="stat-value">{{ correctCount }} / {{ totalCount }}</span>
          <span class="stat-label">答对题数</span>
        </div>
        <div class="stat-card">
          <span class="stat-value" :class="averageScore >= 60 ? 'good' : 'bad'">{{ averageScore }}</span>
          <span class="stat-label">平均分</span>
        </div>
      </div>

      <!-- 逐题明细 -->
      <div class="result-list">
        <div
          v-for="(r, idx) in trainingResults"
          :key="idx"
          class="result-item"
          :class="{
            'result-correct': r.evaluation.is_correct === true || (r.evaluation.is_correct === null && r.evaluation.score >= 60),
            'result-wrong': r.evaluation.is_correct === false || (r.evaluation.is_correct === null && r.evaluation.score < 60)
          }"
        >
          <!-- 左侧：序号 + 状态图标 -->
          <div class="result-index">
            <span class="idx-num">{{ idx + 1 }}</span>
            <span class="idx-icon">
              {{ r.evaluation.is_correct === true || (r.evaluation.is_correct === null && r.evaluation.score >= 60) ? '✅' : '❌' }}
            </span>
          </div>

          <!-- 中间：题目内容 + 答题详情 -->
          <div class="result-body">
            <p class="result-question">{{ r.question.content }}</p>
            <div class="result-meta">
              <span class="result-tag">{{ r.question.tags }}</span>
              <span class="result-type">{{ r.question.question_type }}</span>
            </div>
            <div class="result-answers">
              <span class="your-answer">你的答案：{{ r.userAnswer }}</span>
              <span v-if="r.evaluation.is_correct === false" class="std-answer">
                正确答案：{{ r.evaluation.standard_answer }}
              </span>
            </div>
            <p class="result-comment">{{ r.evaluation.ai_comment }}</p>
          </div>

          <!-- 右侧：分数 -->
          <div class="result-score">
            <span :class="r.evaluation.score >= 60 ? 'score-good' : 'score-bad'">
              {{ r.evaluation.score }}
            </span>
            <span class="score-unit">分</span>
          </div>
        </div>
      </div>

      <button class="restart-btn" @click="restartTraining">再来一套！</button>
    </div>
  </div>
</template>

<style scoped>
/* ===== 容器基础 ===== */
.training-container {
  height: 100%;
  display: flex;
  flex-direction: column;
}

/* ===== Setup 页 ===== */
.setup-room {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 40px 24px;
}
.setup-content {
  width: 100%;
  max-width: 560px;
  display: flex;
  flex-direction: column;
  gap: 32px;
}
.setup-title-group {
  text-align: center;
}
.setup-title {
  font-size: 2rem;
  font-weight: 700;
  color: #2c3e50;
  margin: 0 0 8px;
}
.setup-subtitle {
  color: #888;
  margin: 0;
  font-size: 0.95rem;
}
.setup-panel {
  background: #f4f6f8;
  border-radius: 14px;
  padding: 24px;
}
.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
}
.panel-label {
  font-weight: 600;
  color: #2c3e50;
  font-size: 0.95rem;
}
.selected-hint {
  font-size: 0.82rem;
  color: #42b983;
  font-weight: 600;
}
.tag-grid {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}
.tag-btn {
  padding: 9px 18px;
  cursor: pointer;
  border-radius: 8px;
  border: 1.5px solid #dde1e7;
  background: #fff;
  color: #555;
  font-size: 0.9rem;
  transition: all 0.18s;
  font-weight: 500;
}
.tag-btn:hover { border-color: #42b983; color: #42b983; background: #f0f9f4; }
.tag-btn.selected { background-color: #42b983; color: #fff; border-color: #42b983; }

.start-btn {
  width: 100%;
  font-size: 1.05rem;
  font-weight: 600;
  padding: 15px 24px;
  background-color: #646cff;
  color: white;
  border: none;
  border-radius: 10px;
  cursor: pointer;
  transition: background 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}
.start-btn:hover:not(:disabled) { background-color: #535bf2; }
.start-btn:disabled { background: #c5c7d4; cursor: not-allowed; }
.start-hint { font-size: 0.85rem; opacity: 0.85; font-weight: 400; }
.question-box { background: #f4f6f8; padding: 20px; border-radius: 8px; margin-bottom: 20px; border-left: 4px solid #42b983;}
.question-text { font-size: 1.1rem; font-weight: bold; color: #2c3e50; margin: 10px 0;}
.meta { font-size: 0.85em; color: #888; }
textarea { width: 100%; height: 120px; padding: 12px; border-radius: 8px; border: 1px solid #ddd; resize: vertical; margin-bottom: 10px;}
.multi-hint { font-size: 0.82rem; color: #888; margin: 0 0 10px; }
.submit-btn { background: #000; color: #fff; border: none; padding: 10px 20px; border-radius: 6px; cursor: pointer; }
.submit-btn:disabled { background: #ccc; cursor: not-allowed; }
.result-box { background: #fafafa; padding: 20px; border-radius: 8px; margin-top: 20px; border: 1px solid #eee;}
.score { color: #42b983; font-size: 1.3rem; font-weight: bold; }
.next-btn { margin-top: 15px; background-color: #f0ad4e; color: white; border: none; padding: 8px 20px; border-radius: 6px; cursor: pointer;}
/* 在 <style scoped> 里追加 */  
.correct { color: #42b983; font-weight: bold; }  
.wrong   { color: #e74c3c; font-weight: bold; }  
.answer-box {  
  background: #f0f9f4;  
  border-left: 4px solid #42b983;  
  padding: 10px 15px;  
  border-radius: 6px;  
  margin: 10px 0;  
}  
.explanation-box {  
  background: #f8f9ff;  
  border-left: 4px solid #646cff;  
  padding: 10px 15px;  
  border-radius: 6px;  
  margin: 10px 0;  
}  
.answer-box p, .explanation-box p {
  margin: 6px 0 0;
  line-height: 1.6;
  color: #2c3e50;
}

/* ===== 总结页样式 ===== */
.summary-room {
  padding: 30px 24px;
  max-width: 800px;
  margin: 0 auto;
}
.summary-header {
  text-align: center;
  margin-bottom: 28px;
}
.summary-header h2 {
  font-size: 1.8rem;
  color: #2c3e50;
  margin: 0 0 6px;
}
.summary-subtitle {
  color: #888;
  margin: 0;
}

/* 统计卡片区 */
.stat-cards {
  display: flex;
  gap: 16px;
  margin-bottom: 32px;
}
.stat-card {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 20px 12px;
  background: #f4f6f8;
  border-radius: 12px;
  gap: 6px;
}
.stat-value {
  font-size: 2rem;
  font-weight: bold;
  color: #2c3e50;
}
.stat-value.good { color: #42b983; }
.stat-value.bad  { color: #e74c3c; }
.stat-label {
  font-size: 0.85rem;
  color: #888;
}

/* 逐题明细列表 */
.result-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-bottom: 32px;
}
.result-item {
  display: flex;
  align-items: flex-start;
  gap: 14px;
  padding: 16px;
  border-radius: 10px;
  border: 1px solid #eee;
  background: #fff;
  border-left-width: 4px;
}
.result-correct { border-left-color: #42b983; }
.result-wrong   { border-left-color: #e74c3c; }

.result-index {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  min-width: 32px;
}
.idx-num {
  font-size: 0.75rem;
  color: #aaa;
}
.idx-icon {
  font-size: 1.1rem;
}

.result-body {
  flex: 1;
  min-width: 0;
}
.result-question {
  font-weight: 600;
  color: #2c3e50;
  margin: 0 0 6px;
  font-size: 0.95rem;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.result-meta {
  display: flex;
  gap: 8px;
  margin-bottom: 8px;
}
.result-tag, .result-type {
  font-size: 0.75rem;
  padding: 2px 8px;
  border-radius: 4px;
  background: #f0f2f5;
  color: #666;
}
.result-answers {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  font-size: 0.85rem;
  margin-bottom: 6px;
}
.your-answer { color: #555; }
.std-answer  { color: #e74c3c; font-weight: 600; }
.result-comment {
  font-size: 0.85rem;
  color: #777;
  margin: 0;
  line-height: 1.5;
}

.result-score {
  display: flex;
  flex-direction: column;
  align-items: center;
  min-width: 48px;
}
.score-good { font-size: 1.5rem; font-weight: bold; color: #42b983; }
.score-bad  { font-size: 1.5rem; font-weight: bold; color: #e74c3c; }
.score-unit { font-size: 0.75rem; color: #aaa; }

/* 重新选题按钮 */
.restart-btn {
  display: block;
  width: 100%;
  padding: 14px;
  font-size: 1rem;
  font-weight: 600;
  background-color: #646cff;
  color: #fff;
  border: none;
  border-radius: 10px;
  cursor: pointer;
  transition: background 0.2s;
}
.restart-btn:hover { background-color: #535bf2; }
</style>