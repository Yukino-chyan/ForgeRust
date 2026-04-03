<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

interface AiResponse {
  score: number;
  comment: string;
  next_topic_suggestion: string;
}

interface Question {
  id: number;
  content: string;
  tags: string;
  difficulty: number;
}
const tags = ["操作系统", "计算机网络", "Java后端"];
const appState = ref<'setup' | 'interview'>('setup'); // 控制当前显示哪个页面
const selectedTags = ref<string[]>([]); // 用户选中的考点（现在是数组了，支持多选）
const questionList = ref<Question[]>([]); // 后端发来的一整套试卷
const currentIndex = ref(0); // 当前做到第几题了
const currentQuestion = ref<Question | null>(null);
const userAnswer = ref("");
const aiResult = ref<AiResponse | null>(null);
const isLoading = ref(false);

// 负责点击标签时的选中/取消选中逻辑
function toggleTag(tag: string) {
  const index = selectedTags.value.indexOf(tag);
  if (index > -1) {
    selectedTags.value.splice(index, 1); // 再次点击取消选中
  } else {
    selectedTags.value.push(tag); // 选中
  }
}

// 2. 负责点击“开始面试”按钮的逻辑
async function startInterview() {
  if (selectedTags.value.length === 0) {
    return alert("请至少选择一个考点！");
  }

  try {
    questionList.value = await invoke("generate_interview", { tags: selectedTags.value });
    
    // 初始化考试状态
    currentIndex.value = 0; 
    currentQuestion.value = questionList.value[0]; // 把试卷的第一题挂载到当前显示
    userAnswer.value = ""; 
    aiResult.value = null;  
    // 状态机切换！
    appState.value = 'interview'; 
    
  } catch (error) {
    alert(error);
  }
}

async function startEvaluation() {
    isLoading.value = true;
  try {
    aiResult.value = await invoke("mock_evaluate_answer", { 
      answer: userAnswer.value 
    });
  } catch (error) {
    console.error("调用失败:", error);
  } finally {
    isLoading.value = false;
  }
}

async function handleImport() {
  try {
    // 唤起操作系统的文件选择框
    const selectedPath = await open({
      multiple: false,
      filters: [{
        name: '题库文件',
        extensions: ['txt', 'json', 'csv']
      }]
    });

    if (!selectedPath) {
      console.log("用户取消了文件选择");
      return; 
    }
    // 发送给 Rust 后端去处理
    console.log("选中的文件路径：", selectedPath);
    const resultMsg = await invoke("import_questions_from_file", {
      filePath: typeof selectedPath === 'string' ? selectedPath : (selectedPath as any).path
    });

    alert(`🎉 ${resultMsg}\n\n(现在你可以多点几次标签抽题，看看能不能抽到刚才导入的题了)`);

  } catch (error) {
    console.error("文件导入失败:", error);
    alert(`导入出错: ${error}`);
  }
}

// 切换到下一题，或者结束面试
function nextQuestion() {
  if (currentIndex.value < questionList.value.length - 1) {
    // 还没考完，进入下一题
    currentIndex.value++;
    currentQuestion.value = questionList.value[currentIndex.value];
    userAnswer.value = ""; // 清空上一题的答案
    aiResult.value = null; // 清空上一题的评价
  } else {
    // 考完了，交卷！
    alert("🎉 面试结束！你已经完成了所有题目。");
    appState.value = 'setup'; // 回到备考室
    selectedTags.value = [];  
  }
}

</script>

<template>
  <main class="container">
    <div class="header">
      <h1>ForgeRust 面试演练</h1>
      <button class="import-btn" @click="handleImport">📁 导入题库</button>
    </div>

    <div v-if="appState === 'setup'">
      <section class="tag-section">
        <span>选择考点（可多选）：</span>
        <button 
          v-for="tag in tags" 
          :key="tag" 
          @click="toggleTag(tag)"
          :class="['tag-btn', { 'selected': selectedTags.includes(tag) }]"
        >
          {{ tag }}
        </button>
      </section>
      
      <button class="start-btn" @click="startInterview">🚀 开始面试</button>
    </div>

    <div v-else-if="appState === 'interview'">
      <section class="question-box">
        <h3>🎙️ 面试官提问 ({{ currentIndex + 1 }} / {{ questionList.length }})：</h3>
        <p class="question-text">{{ currentQuestion?.content }}</p>
        <span class="meta">
          标签：{{ currentQuestion?.tags }} | 难度：{{ currentQuestion?.difficulty }}
        </span>
      </section>

      <section>
        <textarea 
          v-model="userAnswer" 
          placeholder="请在这里输入你的回答..."
          :disabled="isLoading"
        ></textarea>
        <br />
        <button @click="startEvaluation" :disabled="isLoading || !userAnswer">
          {{ isLoading ? "面试官思考中 (2s)..." : "提交回答" }}
        </button>
      </section>

      <section v-if="aiResult" class="result-box">
        <hr />
        <h3>📈 面试评价</h3>
        <p><strong>得分：</strong> <span class="score">{{ aiResult.score }}</span></p>
        <p><strong>评语：</strong> {{ aiResult.comment }}</p>
        <p><strong>后续建议：</strong> {{ aiResult.next_topic_suggestion }}</p>
        
        <button class="next-btn" @click="nextQuestion">👉 下一题</button>
      </section>
    </div>

  </main>
</template>

<style scoped>
.logo.vite:hover {
  filter: drop-shadow(0 0 2em #747bff);
}

.logo.vue:hover {
  filter: drop-shadow(0 0 2em #249b73);
}

</style>
<style>
:root {
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  font-size: 16px;
  line-height: 24px;
  font-weight: 400;

  color: #0f0f0f;
  background-color: #f6f6f6;

  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}

.container {
  margin: 0;
  padding-top: 10vh;
  display: flex;
  flex-direction: column;
  justify-content: center;
  text-align: center;
}

.logo {
  height: 6em;
  padding: 1.5em;
  will-change: filter;
  transition: 0.75s;
}

.logo.tauri:hover {
  filter: drop-shadow(0 0 2em #24c8db);
}

.row {
  display: flex;
  justify-content: center;
}

a {
  font-weight: 500;
  color: #646cff;
  text-decoration: inherit;
}

a:hover {
  color: #535bf2;
}

h1 {
  text-align: center;
}

input,
button {
  border-radius: 8px;
  border: 1px solid transparent;
  padding: 0.6em 1.2em;
  font-size: 1em;
  font-weight: 500;
  font-family: inherit;
  color: #0f0f0f;
  background-color: #ffffff;
  transition: border-color 0.25s;
  box-shadow: 0 2px 2px rgba(0, 0, 0, 0.2);
}

button {
  cursor: pointer;
}

button:hover {
  border-color: #396cd8;
}
button:active {
  border-color: #396cd8;
  background-color: #e8e8e8;
}

input,
button {
  outline: none;
}

#greet-input {
  margin-right: 5px;
}

@media (prefers-color-scheme: dark) {
  :root {
    color: #f6f6f6;
    background-color: #2f2f2f;
  }

  a:hover {
    color: #24c8db;
  }

  input,
  button {
    color: #ffffff;
    background-color: #0f0f0f98;
  }
  button:active {
    background-color: #0f0f0f69;
  }
}
/* 选中的考点变成绿色高亮 */
.tag-btn.selected {
  background-color: #42b983;
  color: white;
  border-color: #42b983;
}
/* 开始面试和下一题按钮的样式 */
.start-btn { margin-top: 20px; font-size: 1.1rem; padding: 10px 20px; background-color: #646cff; color: white; border: none; border-radius: 8px;}
.next-btn { margin-top: 15px; background-color: #f0ad4e; color: white; border: none; padding: 8px 16px; border-radius: 6px; }
</style>