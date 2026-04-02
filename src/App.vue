<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

const tags = ["操作系统", "计算机网络", "Java后端"];
const currentQuestion = ref<any>(null);
const userAnswer = ref("");
const aiResult = ref(null);
const isLoading = ref(false);

async function fetchQuestion(tag: string) {
  aiResult.value = null; 
  userAnswer.value = ""; 
  currentQuestion.value = await invoke("get_mock_question", { tag });
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

</script>

<template>
  <main class="container">
    <h1>ForgeRust 面试演练</h1>

    <section class="tag-section">
      <span>选择考点：</span>
      <button 
        v-for="tag in tags" 
        :key="tag" 
        @click="fetchQuestion(tag)"
        class="tag-btn"
      >
        {{ tag }}
      </button>
    </section>

    <section v-if="currentQuestion" class="question-box">
      <h3>🎙️ 面试官提问：</h3>
      <p class="question-text">{{ currentQuestion.content }}</p>
      <span class="meta">
        标签：{{ currentQuestion.tags }} | 难度：{{ currentQuestion.difficulty }}
      </span>
    </section>

    <section v-if="currentQuestion">
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
    </section>
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

</style>