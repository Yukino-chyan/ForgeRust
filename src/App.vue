<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";

import QuestionTraining from "./components/QuestionTraining.vue";

const currentView = ref<'training' | 'mock_interview'>('training');
const isImporting = ref(false);
const progress = ref({ current: 0, total: 0, message: "", is_finished: false });

async function handleImport() {
  const selected = await open({
    multiple: false,
    filters: [{ name: 'JSON题库', extensions: ['json'] }]
  });

  if (!selected) return;
  const path = typeof selected === 'string' ? selected : (selected as any).path;

  const unlisten = await listen("import-status", (event: any) => {
    const data = event.payload;
    progress.value = data;
    isImporting.value = !data.is_finished;
    if (data.is_finished) unlisten();
  });

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
    <!-- 左侧导航栏 -->
    <aside class="sidebar">
      <div class="logo">
        <div class="logo-icon">F</div>
        <div class="logo-text">
          <h2>ForgeRust</h2>
          <span class="version">v1.0 · 面试备考</span>
        </div>
      </div>

      <div class="divider"></div>

      <nav class="nav-menu">
        <button
          :class="['nav-item', { active: currentView === 'training' }]"
          @click="currentView = 'training'"
        >
          <span class="nav-icon">📝</span>
          <span>题库训练</span>
        </button>
        <button
          :class="['nav-item', { active: currentView === 'mock_interview' }]"
          @click="currentView = 'mock_interview'"
        >
          <span class="nav-icon">🤖</span>
          <span>模拟面试</span>
          <span class="badge">即将上线</span>
        </button>
      </nav>

      <div class="sidebar-footer">
        <button class="import-btn" @click="handleImport" :disabled="isImporting">
          <span>{{ isImporting ? '⏳' : '📁' }}</span>
          <span>{{ isImporting ? '导入中...' : '导入题库' }}</span>
        </button>
      </div>
    </aside>

    <!-- 右侧内容区 -->
    <main class="main-content">
      <div v-if="currentView === 'training'" class="view-wrapper">
        <QuestionTraining />
      </div>

      <div v-else-if="currentView === 'mock_interview'" class="placeholder-box">
        <div class="placeholder-inner">
          <div class="placeholder-icon">🤖</div>
          <h2>AI 模拟面试</h2>
          <p>基于大语言模型的动态追问式面试体验</p>
          <p class="coming-soon">敬请期待</p>
        </div>
      </div>
    </main>

    <!-- 导入进度浮层 -->
    <Transition name="toast">
      <div v-if="isImporting || progress.is_finished" class="import-toast">
        <div class="toast-header">
          <span>{{ progress.is_finished ? '✅ 导入完成' : '⚙️ 正在导入题库' }}</span>
          <button v-if="progress.is_finished" class="toast-close" @click="progress.is_finished = false">×</button>
        </div>
        <div class="toast-bar-bg">
          <div
            class="toast-bar"
            :style="{ width: progress.total ? `${(progress.current / progress.total) * 100}%` : '0%' }"
          ></div>
        </div>
        <p class="toast-msg">{{ progress.message }}</p>
      </div>
    </Transition>
  </div>
</template>

<style>
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
body, html {
  height: 100%;
  font-family: 'Inter', 'PingFang SC', 'Microsoft YaHei', sans-serif;
  background: #080d18;
  color: #e2e8f0;
  -webkit-font-smoothing: antialiased;
}
::-webkit-scrollbar { width: 6px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: rgba(99,179,237,0.25); border-radius: 3px; }
</style>

<style scoped>
.app-layout {
  display: flex;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  background: #080d18;
}

/* ===== 侧边栏 ===== */
.sidebar {
  width: 240px;
  flex-shrink: 0;
  background: linear-gradient(180deg, #0d1529 0%, #0a1020 100%);
  border-right: 1px solid rgba(99,179,237,0.1);
  display: flex;
  flex-direction: column;
  padding: 24px 16px;
  position: relative;
  z-index: 10;
}

.logo {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 4px 0px;
  margin-bottom: 24px;
}
.logo-icon {
  width: 40px;
  height: 40px;
  border-radius: 10px;
  background: linear-gradient(135deg, #4facfe, #00d4ff);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 1.3rem;
  font-weight: 900;
  color: #080d18;
  box-shadow: 0 0 16px rgba(79,172,254,0.4);
  flex-shrink: 0;
}
.logo-text h2 {
  font-size: 1.1rem;
  font-weight: 700;
  background: linear-gradient(90deg, #4facfe, #00d4ff);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  line-height: 1.2;
}
.version {
  font-size: 0.68rem;
  color: #4a5568;
  letter-spacing: 0.02em;
}

.divider {
  height: 1px;
  background: linear-gradient(90deg, transparent, rgba(99,179,237,0.2), transparent);
  margin-bottom: 20px;
}

.nav-menu {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 11px 14px;
  background: transparent;
  border: none;
  border-radius: 10px;
  color: #718096;
  font-size: 0.9rem;
  cursor: pointer;
  transition: all 0.2s ease;
  text-align: left;
  width: 100%;
  position: relative;
}
.nav-item:hover {
  background: rgba(79,172,254,0.08);
  color: #90cdf4;
}
.nav-item.active {
  background: linear-gradient(135deg, rgba(79,172,254,0.18), rgba(0,212,255,0.1));
  color: #4facfe;
  font-weight: 600;
  box-shadow: inset 0 0 0 1px rgba(79,172,254,0.25);
}
.nav-icon { font-size: 1rem; }
.badge {
  margin-left: auto;
  font-size: 0.6rem;
  padding: 2px 6px;
  border-radius: 4px;
  background: rgba(113,128,150,0.2);
  color: #718096;
  white-space: nowrap;
}

.sidebar-footer {
  padding-top: 16px;
  border-top: 1px solid rgba(99,179,237,0.08);
}
.import-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  width: 100%;
  padding: 10px;
  background: transparent;
  border: 1px dashed rgba(79,172,254,0.35);
  border-radius: 10px;
  color: #4facfe;
  font-size: 0.875rem;
  cursor: pointer;
  transition: all 0.2s ease;
}
.import-btn:hover:not(:disabled) {
  background: rgba(79,172,254,0.08);
  border-color: rgba(79,172,254,0.6);
  box-shadow: 0 0 12px rgba(79,172,254,0.15);
}
.import-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* ===== 主内容区 ===== */
.main-content {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.view-wrapper {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* ===== 占位页 ===== */
.placeholder-box {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}
.placeholder-inner {
  text-align: center;
  animation: fadeInUp 0.5s ease;
}
.placeholder-icon { font-size: 4rem; margin-bottom: 20px; }
.placeholder-inner h2 {
  font-size: 1.6rem;
  color: #e2e8f0;
  margin-bottom: 10px;
}
.placeholder-inner p { color: #718096; font-size: 0.95rem; }
.coming-soon {
  display: inline-block;
  margin-top: 20px;
  padding: 6px 18px;
  border-radius: 20px;
  background: rgba(79,172,254,0.1);
  border: 1px solid rgba(79,172,254,0.25);
  color: #4facfe !important;
  font-size: 0.82rem !important;
}

/* ===== 导入进度条 ===== */
.import-toast {
  position: fixed;
  bottom: 24px;
  right: 24px;
  width: 320px;
  background: rgba(13,21,41,0.95);
  border: 1px solid rgba(79,172,254,0.25);
  border-radius: 14px;
  padding: 16px;
  box-shadow: 0 8px 32px rgba(0,0,0,0.4), 0 0 0 1px rgba(79,172,254,0.1);
  backdrop-filter: blur(12px);
  z-index: 1000;
}
.toast-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 0.85rem;
  color: #90cdf4;
  font-weight: 600;
  margin-bottom: 10px;
}
.toast-close {
  background: none;
  border: none;
  color: #4a5568;
  cursor: pointer;
  font-size: 1.1rem;
  line-height: 1;
  padding: 0 2px;
}
.toast-close:hover { color: #90cdf4; }
.toast-bar-bg {
  height: 4px;
  background: rgba(255,255,255,0.08);
  border-radius: 2px;
  overflow: hidden;
  margin-bottom: 8px;
}
.toast-bar {
  height: 100%;
  background: linear-gradient(90deg, #4facfe, #00d4ff);
  border-radius: 2px;
  transition: width 0.4s ease;
  box-shadow: 0 0 8px rgba(79,172,254,0.6);
}
.toast-msg {
  font-size: 0.78rem;
  color: #4a5568;
  line-height: 1.4;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ===== 动画 ===== */
.toast-enter-active, .toast-leave-active { transition: all 0.3s ease; }
.toast-enter-from, .toast-leave-to { opacity: 0; transform: translateY(16px); }

@keyframes fadeInUp {
  from { opacity: 0; transform: translateY(20px); }
  to   { opacity: 1; transform: translateY(0); }
}
</style>
