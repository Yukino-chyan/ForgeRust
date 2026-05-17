<script setup lang="ts">
import { ref, onMounted, provide, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Sidebar from "./Sidebar.vue";
import type { View } from "./Sidebar.vue";

import Dashboard from "../Dashboard.vue";
import QuestionTraining from "../QuestionTraining.vue";
import WrongBook from "../WrongBook.vue";
import AIGenerate from "../AIGenerate.vue";
import QuestionLibrary from "../QuestionLibrary.vue";
import Settings from "../Settings.vue";
import Icon from "../ui/Icon.vue";

import { useImportProgress } from "../../composables/useImportProgress";

const currentView = ref<View>("dashboard");
const wrongPracticeIds = ref<number[]>([]);
const trainingState = ref<"setup" | "interview" | "summary">("setup");
const pendingView = ref<View | null>(null);
const showNavWarning = ref(false);

const apiKey = ref("");
const apiUrl = ref("https://zenmux.ai/api/v1/chat/completions");

const apiConfigured = computed(() => apiKey.value.trim().length > 0);

provide("apiKey", apiKey);
provide("apiUrl", apiUrl);

onMounted(async () => {
  try {
    const cfg: any = await invoke("get_api_config");
    apiKey.value = cfg.api_key ?? "";
    apiUrl.value = cfg.api_url ?? apiUrl.value;
  } catch (e) {
    console.warn("加载 API 配置失败", e);
  }
});

function handleStateChange(state: "setup" | "interview" | "summary") {
  trainingState.value = state;
  if (state !== "interview") showNavWarning.value = false;
}

function navigateTo(view: View) {
  if (currentView.value === "training" && trainingState.value === "interview") {
    pendingView.value = view;
    showNavWarning.value = true;
    return;
  }
  showNavWarning.value = false;
  currentView.value = view;
}

function confirmNavAway() {
  if (pendingView.value) currentView.value = pendingView.value;
  pendingView.value = null;
  showNavWarning.value = false;
  trainingState.value = "setup";
}
function cancelNavAway() {
  pendingView.value = null;
  showNavWarning.value = false;
}

function handleWrongPractice(ids: number[]) {
  wrongPracticeIds.value = ids;
  currentView.value = "training";
}

function handleDashboardAction(payload: { mode: "wrong" | "weak" | "random"; ids?: number[] }) {
  if (payload.mode === "wrong" && payload.ids) {
    wrongPracticeIds.value = payload.ids;
  }
  currentView.value = "training";
}

const { state: importState, dismiss: dismissImport } = useImportProgress();
const importPercent = computed(() => {
  if (!importState.total) return 0;
  return Math.round((importState.current / importState.total) * 100);
});
</script>

<template>
  <div class="shell">
    <Sidebar
      :current="currentView"
      :api-configured="apiConfigured"
      @navigate="navigateTo"
    />

    <main class="content">
      <Dashboard
        v-show="currentView === 'dashboard'"
        :is-active="currentView === 'dashboard'"
        @start-training="handleDashboardAction"
        @navigate="(v: string) => navigateTo(v as View)"
      />

      <QuestionTraining
        v-show="currentView === 'training'"
        :wrong-practice-ids="wrongPracticeIds"
        :is-active="currentView === 'training'"
        @consumed="wrongPracticeIds = []"
        @state-change="handleStateChange"
      />

      <WrongBook
        v-show="currentView === 'wrong_book'"
        :is-active="currentView === 'wrong_book'"
        @start-wrong-practice="handleWrongPractice"
      />

      <AIGenerate
        v-show="currentView === 'ai_generate'"
        :is-active="currentView === 'ai_generate'"
      />

      <QuestionLibrary v-show="currentView === 'question_library'" />

      <Settings v-show="currentView === 'settings'" />

      <div v-show="currentView === 'mock_interview'" class="coming-soon">
        <Icon name="MessageSquare" :size="48" :stroke-width="1.5" />
        <h2>AI 模拟面试</h2>
        <p>基于大语言模型的动态追问式面试体验。</p>
        <span class="coming-soon-chip">敬请期待</span>
      </div>
    </main>

    <!-- 训练中导航拦截 -->
    <Transition name="modal">
      <div v-if="showNavWarning" class="modal-backdrop" @click.self="cancelNavAway">
        <div class="modal">
          <div class="modal-icon"><Icon name="AlertTriangle" :size="20" /></div>
          <h3 class="modal-title">离开当前训练？</h3>
          <p class="modal-body">训练进行中，离开将丢失当前进度，且不会记入历史。</p>
          <div class="modal-actions">
            <button class="fr-btn fr-btn-ghost" @click="cancelNavAway">继续训练</button>
            <button class="fr-btn fr-btn-danger" @click="confirmNavAway">确定离开</button>
          </div>
        </div>
      </div>
    </Transition>

    <!-- 导入进度 toast -->
    <Transition name="toast">
      <div v-if="importState.active || importState.finished" class="toast">
        <div class="toast-head">
          <Icon :name="importState.finished ? 'Check' : 'Loader2'" :size="16" :class="{ spin: !importState.finished }" />
          <span>{{ importState.finished ? '导入完成' : '正在导入题库' }}</span>
          <button v-if="importState.finished" class="toast-close" @click="dismissImport">
            <Icon name="X" :size="14" />
          </button>
        </div>
        <div class="toast-bar"><div class="toast-bar-fill" :style="{ width: importPercent + '%' }"></div></div>
        <p class="toast-msg">{{ importState.message }}</p>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.shell {
  display: flex;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  background: var(--bg);
}
.content {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-width: 0;
}

.coming-soon {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--sp-3);
  color: var(--text-muted);
}
.coming-soon h2 {
  font-size: var(--fs-20);
  font-weight: var(--fw-semibold);
  color: var(--text);
}
.coming-soon p { font-size: var(--fs-13); }
.coming-soon-chip {
  margin-top: var(--sp-2);
  padding: 4px 12px;
  border-radius: 999px;
  border: 1px solid var(--border);
  font-size: var(--fs-12);
  color: var(--text-muted);
  background: var(--surface);
}

/* Modal */
.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}
.modal {
  width: 360px;
  background: var(--surface);
  border-radius: var(--radius-lg);
  padding: var(--sp-6);
  box-shadow: var(--shadow-md);
  border: 1px solid var(--border);
}
.modal-icon {
  width: 36px;
  height: 36px;
  border-radius: var(--radius-md);
  background: var(--warning-soft);
  color: var(--warning);
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: var(--sp-3);
}
.modal-title {
  font-size: var(--fs-16);
  font-weight: var(--fw-semibold);
  color: var(--text);
  margin-bottom: var(--sp-2);
}
.modal-body {
  font-size: var(--fs-13);
  color: var(--text-muted);
  margin-bottom: var(--sp-4);
  line-height: 1.5;
}
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--sp-2);
}
.modal-enter-active, .modal-leave-active { transition: opacity var(--dur-base) var(--ease); }
.modal-enter-from, .modal-leave-to { opacity: 0; }

/* Toast */
.toast {
  position: fixed;
  bottom: 20px;
  right: 20px;
  width: 320px;
  padding: var(--sp-4);
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-md);
  z-index: 200;
}
.toast-head {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  font-size: var(--fs-13);
  font-weight: var(--fw-medium);
  color: var(--text);
  margin-bottom: var(--sp-3);
}
.toast-close {
  margin-left: auto;
  color: var(--text-subtle);
  padding: 2px;
  border-radius: var(--radius-sm);
}
.toast-close:hover { color: var(--text); background: var(--surface-2); }
.toast-bar {
  height: 4px;
  background: var(--surface-2);
  border-radius: 2px;
  overflow: hidden;
  margin-bottom: var(--sp-2);
}
.toast-bar-fill {
  height: 100%;
  background: var(--accent);
  transition: width var(--dur-base) var(--ease);
}
.toast-msg {
  font-size: var(--fs-12);
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.toast-enter-active, .toast-leave-active { transition: all var(--dur-base) var(--ease); }
.toast-enter-from, .toast-leave-to { opacity: 0; transform: translateY(12px); }

.spin {
  animation: spin 1s linear infinite;
}
@keyframes spin {
  to { transform: rotate(360deg); }
}
</style>
