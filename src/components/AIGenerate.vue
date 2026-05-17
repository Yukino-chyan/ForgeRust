<script setup lang="ts">
import { ref, inject, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Icon from "./ui/Icon.vue";

interface GeneratedQuestion {
  question_type: string;
  content: string;
  options: string[] | null;
  standard_answer: string;
  explanation: string;
  tags: string;
  difficulty: number;
}

interface GenerateProgress {
  current: number;
  total: number;
  question: GeneratedQuestion | null;
  message: string;
  is_finished: boolean;
  error: string | null;
}

interface PreviewQuestion extends GeneratedQuestion {
  selected: boolean;
  failed: boolean;
  errorMsg: string;
}

const apiKey = inject<any>("apiKey");

const TOPICS = ["Java", "Rust", "操作系统", "计算机网络", "数据库", "数据结构", "其他"];
const TYPES = [
  { value: "ESSAY",  label: "简答题" },
  { value: "SINGLE", label: "单选题" },
  { value: "MULTI",  label: "多选题" },
];
const DIFFICULTIES = [
  { value: 1, label: "入门" },
  { value: 2, label: "初级" },
  { value: 3, label: "中级" },
  { value: 4, label: "高级" },
  { value: 5, label: "专家" },
];

const topic = ref("Java");
const questionType = ref("ESSAY");
const difficulty = ref(3);
const count = ref(5);
const requirement = ref("");

const REQUIREMENT_MAX = 400;
const requirementOverflow = computed(() => requirement.value.length > REQUIREMENT_MAX);

const isGenerating = ref(false);
const progress = ref<GenerateProgress>({ current: 0, total: 0, question: null, message: "", is_finished: false, error: null });
const questions = ref<PreviewQuestion[]>([]);

const saveStatus = ref<"idle" | "saving" | "done">("idle");
const saveCount = ref(0);
const allSelected = ref(true);

function toggleAll() {
  allSelected.value = !allSelected.value;
  questions.value.forEach((q) => { q.selected = allSelected.value; });
}

async function startGenerate() {
  if (!apiKey?.value) {
    alert("请先到「设置」中填写 API Key。");
    return;
  }
  isGenerating.value = true;
  saveStatus.value = "idle";
  questions.value = [];
  progress.value = { current: 0, total: count.value, question: null, message: "准备中...", is_finished: false, error: null };

  const unlisten = await listen<GenerateProgress>("ai-generate-progress", (event) => {
    const data = event.payload;
    progress.value = data;

    if (data.question) {
      questions.value.push({ ...data.question, selected: true, failed: false, errorMsg: "" });
    } else if (data.error) {
      questions.value.push({
        question_type: questionType.value,
        content: `（第 ${data.current} 题生成失败）`,
        options: null,
        standard_answer: "",
        explanation: "",
        tags: topic.value,
        difficulty: difficulty.value,
        selected: false,
        failed: true,
        errorMsg: data.error,
      });
    }

    if (data.is_finished) {
      isGenerating.value = false;
      unlisten();
    }
  });

  try {
    await invoke("generate_questions_by_ai", {
      topic: topic.value,
      questionType: questionType.value,
      difficulty: difficulty.value,
      count: count.value,
      requirement: requirement.value.trim() || null,
    });
  } catch (e) {
    alert(`生成失败: ${e}`);
    isGenerating.value = false;
    unlisten();
  }
}

async function saveSelected() {
  const toSave = questions.value.filter((q) => q.selected && !q.failed);
  if (toSave.length === 0) {
    alert("请至少选择一道有效题目");
    return;
  }
  saveStatus.value = "saving";
  try {
    const saved: number = await invoke("save_ai_generated_questions", { questions: toSave });
    saveCount.value = saved;
    saveStatus.value = "done";
  } catch (e) {
    alert(`保存失败: ${e}`);
    saveStatus.value = "idle";
  }
}

function reset() {
  questions.value = [];
  saveStatus.value = "idle";
  progress.value = { current: 0, total: 0, question: null, message: "", is_finished: false, error: null };
}

function progressPct() {
  if (!progress.value.total) return 0;
  return Math.round((progress.value.current / progress.value.total) * 100);
}

function difficultyStar(n: number) {
  return "★".repeat(n);
}
</script>

<template>
  <div class="fr-page ai-gen">
    <header>
      <h1 class="fr-page-title">AI 出题</h1>
      <p class="fr-page-subtitle">由大语言模型根据你的偏好自动生成面试题，生成后可预览并选择性入库。</p>
    </header>

    <!-- 配置卡 -->
    <section class="fr-card config">
      <div class="field">
        <label>考点方向</label>
        <div class="chip-row">
          <button
            v-for="t in TOPICS"
            :key="t"
            :class="['tag-pill', { active: topic === t }]"
            @click="topic = t"
          >{{ t }}</button>
        </div>
      </div>

      <div class="field">
        <div class="field-head">
          <label>具体要求（可选）</label>
          <span :class="['field-counter fr-mono', { over: requirementOverflow }]">
            {{ requirement.length }} / {{ REQUIREMENT_MAX }}
          </span>
        </div>
        <textarea
          v-model="requirement"
          class="fr-input req-textarea"
          rows="3"
          :maxlength="REQUIREMENT_MAX + 50"
          placeholder="例如：聚焦 Java 并发，要带具体代码片段；或：关于 TCP 拥塞控制，避免出过于基础的握手题。留空则按考点泛出题。"
        ></textarea>
        <p v-if="requirementOverflow" class="field-warn">
          建议精简到 {{ REQUIREMENT_MAX }} 字以内，过长的要求 AI 可能会忽略部分细节。
        </p>
      </div>

      <div class="grid-3">
        <div class="field">
          <label>题型</label>
          <div class="seg">
            <button
              v-for="tp in TYPES"
              :key="tp.value"
              :class="['seg-btn', { active: questionType === tp.value }]"
              @click="questionType = tp.value"
            >{{ tp.label }}</button>
          </div>
        </div>

        <div class="field">
          <label>难度</label>
          <select v-model="difficulty" class="fr-input">
            <option v-for="d in DIFFICULTIES" :key="d.value" :value="d.value">
              {{ d.label }} · {{ difficultyStar(d.value) }}
            </option>
          </select>
        </div>

        <div class="field">
          <label>数量</label>
          <div class="counter">
            <button class="counter-btn" @click="count = Math.max(1, count - 1)" :disabled="count <= 1">−</button>
            <span class="counter-val fr-mono">{{ count }}</span>
            <button class="counter-btn" @click="count = Math.min(10, count + 1)" :disabled="count >= 10">+</button>
          </div>
        </div>
      </div>

      <div class="actions">
        <button class="fr-btn fr-btn-primary" :disabled="isGenerating" @click="startGenerate">
          <Icon :name="isGenerating ? 'Loader2' : 'Sparkles'" :size="14" :class="{ spin: isGenerating }" />
          <span>{{ isGenerating ? "生成中..." : "开始生成" }}</span>
        </button>
        <button v-if="questions.length > 0 && !isGenerating" class="fr-btn fr-btn-ghost" @click="reset">清空</button>
      </div>
    </section>

    <!-- 进度 -->
    <Transition name="fade">
      <div v-if="isGenerating || (progress.total > 0 && !progress.is_finished)" class="fr-card progress">
        <div class="progress-head">
          <span class="progress-msg">{{ progress.message }}</span>
          <span class="progress-ratio fr-mono">{{ progress.current }}/{{ progress.total }}</span>
        </div>
        <div class="progress-bar"><div class="progress-bar-fill" :style="{ width: progressPct() + '%' }"></div></div>
      </div>
    </Transition>

    <!-- 保存成功 -->
    <Transition name="fade">
      <div v-if="saveStatus === 'done'" class="success-banner">
        <Icon name="Check" :size="16" />
        <span>已成功保存 {{ saveCount }} 道题目到题库。</span>
        <button class="fr-btn fr-btn-ghost" @click="reset">再生成一批</button>
      </div>
    </Transition>

    <!-- 预览 -->
    <section v-if="questions.length > 0 && saveStatus !== 'done'" class="preview">
      <div class="preview-head">
        <h3 class="preview-title">
          预览 · <span class="fr-mono">{{ questions.filter(q => !q.failed).length }}</span> 道有效
        </h3>
        <div class="preview-actions">
          <button class="fr-btn fr-btn-ghost" @click="toggleAll">
            {{ allSelected ? "全不选" : "全选" }}
          </button>
          <button
            class="fr-btn fr-btn-primary"
            :disabled="saveStatus === 'saving' || isGenerating"
            @click="saveSelected"
          >
            <Icon name="Save" :size="14" />
            <span>
              {{ saveStatus === "saving"
                ? "保存中..."
                : `保存已选 (${questions.filter(q => q.selected && !q.failed).length})` }}
            </span>
          </button>
        </div>
      </div>

      <div class="q-list">
        <div
          v-for="(q, idx) in questions"
          :key="idx"
          :class="['q-card', { failed: q.failed, selected: q.selected && !q.failed }]"
        >
          <div v-if="q.failed" class="q-failed">
            <Icon name="AlertCircle" :size="14" />
            <span>第 {{ idx + 1 }} 题生成失败：{{ q.errorMsg }}</span>
          </div>

          <template v-else>
            <div class="q-top">
              <label class="q-check">
                <input type="checkbox" v-model="q.selected" />
                <span class="q-num">第 {{ idx + 1 }} 题</span>
              </label>
              <div class="q-badges">
                <span class="fr-chip">{{ TYPES.find(t => t.value === q.question_type)?.label }}</span>
                <span class="fr-chip fr-chip-accent">{{ q.tags }}</span>
                <span class="fr-chip">Lv.{{ q.difficulty }}</span>
              </div>
            </div>

            <div class="q-content">{{ q.content }}</div>

            <div v-if="q.options && q.options.length" class="q-options">
              <div
                v-for="(opt, oi) in q.options"
                :key="oi"
                :class="['q-opt', { correct: opt.startsWith(q.standard_answer[0] ?? '') }]"
              >{{ opt }}</div>
            </div>

            <div class="q-answer">
              <span class="answer-label">答案</span>
              <span class="answer-val fr-mono">{{ q.standard_answer }}</span>
            </div>

            <details class="q-explain">
              <summary>查看解析</summary>
              <p>{{ q.explanation }}</p>
            </details>
          </template>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.ai-gen {
  max-width: var(--content-max);
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: var(--sp-4);
}

.config {
  display: flex;
  flex-direction: column;
  gap: var(--sp-4);
}

.field {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.field > label {
  font-size: var(--fs-12);
  font-weight: var(--fw-medium);
  color: var(--text-muted);
}
.field-head {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
}
.field-counter {
  font-size: 11px;
  color: var(--text-subtle);
}
.field-counter.over { color: var(--warning); }
.req-textarea {
  resize: vertical;
  min-height: 72px;
  font-family: var(--font-sans);
  line-height: 1.5;
}
.field-warn {
  font-size: var(--fs-12);
  color: var(--warning);
  margin-top: 4px;
}

.grid-3 {
  display: grid;
  grid-template-columns: 1.4fr 1fr 1fr;
  gap: var(--sp-4);
}

.chip-row { display: flex; flex-wrap: wrap; gap: 6px; }
.tag-pill {
  padding: 6px 12px;
  border-radius: 999px;
  font-size: var(--fs-13);
  color: var(--text-muted);
  background: var(--surface);
  border: 1px solid var(--border);
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

.seg {
  display: flex;
  gap: 0;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  padding: 2px;
  background: var(--surface-2);
}
.seg-btn {
  flex: 1;
  padding: 6px 8px;
  font-size: var(--fs-13);
  color: var(--text-muted);
  border-radius: 6px;
  transition: background var(--dur-fast) var(--ease), color var(--dur-fast) var(--ease);
}
.seg-btn:hover:not(.active) { color: var(--text); }
.seg-btn.active {
  background: var(--surface);
  color: var(--text);
  font-weight: var(--fw-medium);
  box-shadow: var(--shadow-sm);
}

.counter {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 36px;
}
.counter-btn {
  width: 32px;
  height: 32px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-muted);
  font-size: var(--fs-16);
  transition: all var(--dur-fast) var(--ease);
}
.counter-btn:hover:not(:disabled) {
  border-color: var(--accent);
  color: var(--accent);
}
.counter-btn:disabled { opacity: 0.4; cursor: not-allowed; }
.counter-val {
  min-width: 28px;
  text-align: center;
  font-size: var(--fs-16);
  color: var(--text);
}

.actions {
  display: flex;
  gap: var(--sp-2);
  margin-top: var(--sp-2);
}

.progress {
  padding: var(--sp-4);
}
.progress-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}
.progress-msg { font-size: var(--fs-13); color: var(--text-muted); }
.progress-ratio { font-size: var(--fs-13); color: var(--text); }
.progress-bar {
  height: 4px;
  background: var(--surface-2);
  border-radius: 2px;
  overflow: hidden;
}
.progress-bar-fill {
  height: 100%;
  background: var(--accent);
  transition: width var(--dur-base) var(--ease);
}

.success-banner {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  padding: var(--sp-3) var(--sp-4);
  background: var(--success-soft);
  border: 1px solid var(--success);
  border-radius: var(--radius-lg);
  color: var(--success);
  font-size: var(--fs-13);
  font-weight: var(--fw-medium);
}
.success-banner span { flex: 1; }

.preview {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  overflow: hidden;
}
.preview-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--sp-3) var(--sp-4);
  border-bottom: 1px solid var(--border);
  background: var(--surface-2);
}
.preview-title {
  font-size: var(--fs-13);
  font-weight: var(--fw-semibold);
  color: var(--text);
}
.preview-actions { display: flex; gap: 8px; }

.q-list {
  padding: var(--sp-3);
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
}
.q-card {
  padding: var(--sp-4);
  border-radius: var(--radius-md);
  border: 1px solid var(--border);
  background: var(--surface);
  transition: border-color var(--dur-fast) var(--ease);
}
.q-card.selected { border-color: var(--accent); }
.q-card.failed {
  border-color: var(--danger);
  background: var(--danger-soft);
  color: var(--danger);
}
.q-failed {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--fs-13);
}

.q-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--sp-2);
}
.q-check {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
}
.q-check input { accent-color: var(--accent); width: 14px; height: 14px; cursor: pointer; }
.q-num { font-size: var(--fs-12); color: var(--text-muted); }

.q-badges { display: flex; gap: 4px; flex-wrap: wrap; }

.q-content {
  font-size: var(--fs-14);
  color: var(--text);
  line-height: 1.6;
  margin-bottom: var(--sp-3);
}

.q-options {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: var(--sp-3);
}
.q-opt {
  padding: 8px 12px;
  border-radius: var(--radius-md);
  background: var(--surface-2);
  border: 1px solid var(--border);
  font-size: var(--fs-13);
  color: var(--text-muted);
}
.q-opt.correct {
  background: var(--success-soft);
  border-color: var(--success);
  color: var(--success);
}

.q-answer {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}
.answer-label {
  font-size: 11px;
  font-weight: var(--fw-semibold);
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}
.answer-val {
  font-size: var(--fs-13);
  color: var(--success);
  font-weight: var(--fw-medium);
}

.q-explain {
  border-top: 1px dashed var(--border);
  padding-top: var(--sp-2);
  margin-top: var(--sp-2);
}
.q-explain summary {
  font-size: var(--fs-12);
  color: var(--text-muted);
  cursor: pointer;
  user-select: none;
}
.q-explain summary:hover { color: var(--accent); }
.q-explain p {
  margin-top: 6px;
  font-size: var(--fs-13);
  color: var(--text-muted);
  line-height: 1.6;
}

.fade-enter-active, .fade-leave-active { transition: all var(--dur-base) var(--ease); }
.fade-enter-from, .fade-leave-to { opacity: 0; transform: translateY(-6px); }

.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
</style>
