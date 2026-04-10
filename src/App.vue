<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";

// 引入刚刚写好的组件
import QuestionTraining from "./components/QuestionTraining.vue";

// 全局路由状态：控制右侧显示什么内容
const currentView = ref<'training' | 'mock_interview'>('training');
const isImporting = ref(false);
const progress = ref({ current: 0, total: 0, message: "" });

// 全局工具：导入题库保留在最顶层
async function handleImport() {
  const selected = await open({
    multiple: false,
    filters: [{ name: 'JSON题库', extensions: ['json'] }]
  });
  
  if (!selected) return;
  const path = typeof selected === 'string' ? selected : (selected as any).path;

  // 1. 开始监听后端进度事件
  const unlisten = await listen("import-status", (event: any) => {
    const data = event.payload;
    progress.value = data;
    isImporting.value = !data.is_finished;
    if (data.is_finished) {
       unlisten(); // 任务结束后取消监听
    }
  });

  // 2. 调用后端命令
  try {
    isImporting.value = true;
    await invoke("import_questions_from_file", { filePath: path });
  } catch (e) {
    alert(e);
    isImporting.value = false;
  }
}
</script>

<template>
  <div class="app-layout">
    <aside class="sidebar">
      <div class="logo">
        <h2>ForgeRust</h2>
        <span class="version">v1.0</span>
      </div>

      <nav class="nav-menu">
        <button 
          :class="['nav-item', { active: currentView === 'training' }]" 
          @click="currentView = 'training'"
        >
          📝 题库训练
        </button>
        <button 
          :class="['nav-item', { active: currentView === 'mock_interview' }]" 
          @click="currentView = 'mock_interview'"
        >
          🤖 模拟面试
        </button>
      </nav>

      <div class="sidebar-footer">
        <button class="nav-item import-action" @click="handleImport">
          📁 导入本地题库
        </button>
      </div>
    </aside>

    <main class="main-content">
      <div v-if="currentView === 'training'">
        <QuestionTraining />
      </div>

      <div v-else-if="currentView === 'mock_interview'">
        <div class="placeholder-box">
          <h2>🤖 AI 模拟面试区 (开发中)</h2>
          <p>这里将接入大模型 API，进行无固定题库的自由多轮追问。</p>
          <p>敬请期待...</p>
        </div>
      </div>
      
    </main>
  </div>
</template>

<style>
/* 重置基础样式 */
body, html { margin: 0; padding: 0; height: 100%; font-family: Inter, sans-serif; background-color: #f0f2f5; }
</style>

<style scoped>
/* 核心：左右分栏 Flexbox 布局 */
.app-layout {
  display: flex;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
}

/* 左侧栏样式 */
.sidebar {
  width: 260px;
  background-color: #1a1a2e;
  color: #fff;
  display: flex;
  flex-direction: column;
  padding: 20px 0;
  box-shadow: 2px 0 10px rgba(0,0,0,0.1);
}

.logo { padding: 0 20px 30px; text-align: left; }
.logo h2 { margin: 0; color: #42b983; font-size: 1.8rem;}
.version { font-size: 0.8rem; color: #888; }

.nav-menu { flex: 1; display: flex; flex-direction: column; gap: 10px; padding: 0 15px; }

.nav-item {
  background: transparent; border: none; color: #a0a0b5; text-align: left;
  padding: 12px 15px; font-size: 1.05rem; border-radius: 8px; cursor: pointer; transition: all 0.2s;
}
.nav-item:hover { background-color: rgba(255,255,255,0.05); color: #fff;}
.nav-item.active { background-color: #42b983; color: #fff; font-weight: bold;}
.import-action { color: #f0ad4e; margin: 0 15px; border: 1px dashed #f0ad4e; text-align: center;}
.import-action:hover { background-color: #f0ad4e; color: #1a1a2e; }

/* 右侧内容区样式 */
.main-content {
  flex: 1;
  background-color: #fff;
  margin: 15px;
  border-radius: 12px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.05);
  overflow-y: auto;
}

/* 占位符样式 */
.placeholder-box {
  height: 100%; display: flex; flex-direction: column;
  justify-content: center; align-items: center; color: #666;
}
</style>