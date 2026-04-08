<script setup lang="ts">
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";

// --- 接口定义 ---
interface AiResponse { score: number; comment: string; next_topic_suggestion: string; }
interface Question { 
  id: number; 
  question_type: string; 
  content: string; 
  options: string | null;
  tags: string; 
  difficulty: number; 
  standard_answer: string;
}
// --- 状态变量 ---
const tags = ["操作系统", "计算机网络", "Java后端"];
const appState = ref<'setup' | 'interview'>('setup'); 
const selectedTags = ref<string[]>([]); 
const questionList = ref<Question[]>([]); 
const currentIndex = ref(0); 

const userAnswer = ref("");
const aiResult = ref<AiResponse | null>(null);
const isLoading = ref(false);

const currentQuestion = computed(() => questionList.value[currentIndex.value]);
const currentOptions = computed(() => {
  if (!currentQuestion.value || !currentQuestion.value.options) return [];
  try {
    return JSON.parse(currentQuestion.value.options); // 把 JSON 字符串还原成数组
  } catch (e) {
    return [];
  }
});
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
    aiResult.value = null;
    appState.value = 'interview'; 
  } catch (error) { alert(error); }
}

async function startEvaluation() {
  if (!userAnswer.value) return alert("请先给出你的答案！");
  isLoading.value = true;
  try {
    aiResult.value = await invoke("mock_evaluate_answer", { 
      questionId: currentQuestion.value.id, 
      userAnswer: userAnswer.value
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
    aiResult.value = null; 
  } else {
    alert("🎉 训练结束！你已经完成了所有题目。");
    appState.value = 'setup'; 
    selectedTags.value = [];  
  }
}
</script>

<template>
  <div class="training-container">
    <div v-if="appState === 'setup'" class="setup-room">
      <h2>🎯 题库专项训练</h2>
      <p class="desc">针对特定知识点进行结构化刷题，夯实基础。</p>
      
      <section class="tag-section">
        <button 
          v-for="tag in tags" :key="tag" @click="toggleTag(tag)"
          :class="['tag-btn', { 'selected': selectedTags.includes(tag) }]"
        >
          {{ tag }}
        </button>
      </section>
      
      <button class="start-btn" @click="startInterview">🚀 开始训练</button>
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
              :disabled="isLoading"
            >
            {{ opt }}
          </label>
        </div>

        <div v-else-if="currentQuestion?.question_type === 'ESSAY'">
          <textarea 
            v-model="userAnswer" 
            placeholder="请详细阐述你的回答..." 
            :disabled="isLoading"
          ></textarea>
        </div>

        <br />
        <button class="submit-btn" @click="startEvaluation" :disabled="isLoading || !userAnswer">
          {{ isLoading ? "批阅中..." : "提交回答" }}
        </button>
      </section>

      <section v-if="aiResult" class="result-box">
        <hr />
        <h3>📈 评价反馈</h3>
        <p><strong>得分：</strong> <span class="score">{{ aiResult.score }}</span></p>
        <p><strong>评语：</strong> {{ aiResult.comment }}</p>
        <button class="next-btn" @click="nextQuestion">👉 下一题</button>
      </section>
    </div>
  </div>
</template>

<style scoped>
/* 组件内部专属样式 */
.options-container { display: flex; flex-direction: column; gap: 10px; margin-bottom: 15px;}
.option-label { display: flex; align-items: center; gap: 10px; padding: 10px; background: #fff; border: 1px solid #ddd; border-radius: 6px; cursor: pointer; transition: 0.2s;}
.option-label:hover { border-color: #42b983; background: #f0f9f4;}
.training-container { padding: 20px; text-align: left; }
.desc { color: #666; margin-bottom: 20px; }
.tag-section { margin-bottom: 30px; }
.tag-btn { margin: 0 8px 8px 0; padding: 8px 16px; cursor: pointer; border-radius: 6px; border: 1px solid #ccc; background: #fff; transition: all 0.2s;}
.tag-btn.selected { background-color: #42b983; color: white; border-color: #42b983; }
.start-btn { font-size: 1.1rem; padding: 12px 24px; background-color: #646cff; color: white; border: none; border-radius: 8px; cursor: pointer;}
.question-box { background: #f4f6f8; padding: 20px; border-radius: 8px; margin-bottom: 20px; border-left: 4px solid #42b983;}
.question-text { font-size: 1.1rem; font-weight: bold; color: #2c3e50; margin: 10px 0;}
.meta { font-size: 0.85em; color: #888; }
textarea { width: 100%; height: 120px; padding: 12px; border-radius: 8px; border: 1px solid #ddd; resize: vertical; margin-bottom: 10px;}
.submit-btn { background: #000; color: #fff; border: none; padding: 10px 20px; border-radius: 6px; cursor: pointer; }
.submit-btn:disabled { background: #ccc; cursor: not-allowed; }
.result-box { background: #fafafa; padding: 20px; border-radius: 8px; margin-top: 20px; border: 1px solid #eee;}
.score { color: #42b983; font-size: 1.3rem; font-weight: bold; }
.next-btn { margin-top: 15px; background-color: #f0ad4e; color: white; border: none; padding: 8px 20px; border-radius: 6px; cursor: pointer;}
</style>