<script setup lang="ts">
import { reactive, ref, watch, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Icon from "./Icon.vue";

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

const props = defineProps<{
  mode: "create" | "edit" | "view";
  question?: Question | null;
  tags: string[];
}>();
const emit = defineEmits<{ (e: "close"): void; (e: "saved"): void }>();

const form = reactive({
  question_type: "ESSAY",
  content: "",
  options: [] as string[], // 仅选择题用
  tags: "",
  difficulty: 3,
  standard_answer: "",
  explanation: "",
});
const saving = ref(false);
const errorMsg = ref("");

const isView = computed(() => props.mode === "view");
const isChoice = computed(() => form.question_type !== "ESSAY");
const title = computed(() =>
  props.mode === "create" ? "新增题目" : props.mode === "edit" ? "编辑题目" : "题目详情"
);

function loadFromProp() {
  const q = props.question;
  if (!q) return;
  form.question_type = q.question_type;
  form.content = q.content;
  form.tags = q.tags;
  form.difficulty = q.difficulty;
  form.standard_answer = q.standard_answer;
  form.explanation = q.explanation;
  try {
    form.options = q.options ? JSON.parse(q.options) : [];
  } catch {
    form.options = [];
  }
}
watch(() => props.question, loadFromProp, { immediate: true });

function addOption() {
  form.options.push("");
}
function removeOption(i: number) {
  form.options.splice(i, 1);
}

async function save() {
  if (!form.content.trim()) {
    errorMsg.value = "题目内容不能为空";
    return;
  }
  saving.value = true;
  errorMsg.value = "";
  const optionsArg = isChoice.value && form.options.length
    ? JSON.stringify(form.options.filter((o) => o.trim()))
    : null;
  try {
    if (props.mode === "create") {
      await invoke("create_question", {
        questionType: form.question_type,
        content: form.content,
        options: optionsArg,
        tags: form.tags,
        difficulty: form.difficulty,
        standardAnswer: form.standard_answer,
        explanation: form.explanation,
      });
    } else if (props.mode === "edit" && props.question) {
      await invoke("update_question", {
        id: props.question.id,
        questionType: form.question_type,
        content: form.content,
        options: optionsArg,
        tags: form.tags,
        difficulty: form.difficulty,
        standardAnswer: form.standard_answer,
        explanation: form.explanation,
      });
    }
    emit("saved");
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <div class="modal-mask" @click.self="emit('close')">
    <div class="modal-card fr-card">
      <header class="modal-head">
        <h2>{{ title }}</h2>
        <button class="icon-btn" @click="emit('close')"><Icon name="X" :size="16" /></button>
      </header>

      <div class="modal-body">
        <div class="row">
          <label>题型</label>
          <select v-model="form.question_type" class="fr-input" :disabled="isView">
            <option value="ESSAY">简答</option>
            <option value="SINGLE">单选</option>
            <option value="MULTI">多选</option>
          </select>
        </div>

        <div class="row">
          <label>题目内容</label>
          <textarea v-model="form.content" class="fr-input" rows="3" :disabled="isView"></textarea>
        </div>

        <div v-if="isChoice" class="row">
          <label>选项</label>
          <div class="opt-list">
            <div v-for="(_, i) in form.options" :key="i" class="opt-item">
              <input v-model="form.options[i]" class="fr-input" :disabled="isView" placeholder="A. ..." />
              <button v-if="!isView" class="icon-btn" @click="removeOption(i)"><Icon name="X" :size="14" /></button>
            </div>
            <button v-if="!isView" class="fr-btn fr-btn-ghost" @click="addOption">
              <Icon name="Plus" :size="14" /><span>添加选项</span>
            </button>
          </div>
        </div>

        <div class="row two">
          <div>
            <label>考点标签（逗号分隔）</label>
            <input v-model="form.tags" class="fr-input" :disabled="isView" placeholder="Rust,所有权" />
          </div>
          <div>
            <label>难度 (1-5)</label>
            <input v-model.number="form.difficulty" type="number" min="1" max="5" class="fr-input" :disabled="isView" />
          </div>
        </div>

        <div class="row">
          <label>标准答案</label>
          <textarea v-model="form.standard_answer" class="fr-input" rows="3" :disabled="isView"></textarea>
        </div>

        <div class="row">
          <label>解析</label>
          <textarea v-model="form.explanation" class="fr-input" rows="3" :disabled="isView"></textarea>
        </div>

        <p v-if="errorMsg" class="error-msg">{{ errorMsg }}</p>
      </div>

      <footer class="modal-foot">
        <button class="fr-btn fr-btn-ghost" @click="emit('close')">{{ isView ? "关闭" : "取消" }}</button>
        <button v-if="!isView" class="fr-btn fr-btn-primary" :disabled="saving" @click="save">
          {{ saving ? "保存中..." : "保存" }}
        </button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.modal-mask {
  position: fixed; inset: 0; z-index: 50;
  background: rgba(0,0,0,0.4);
  display: flex; align-items: center; justify-content: center;
  padding: var(--sp-4);
}
.modal-card {
  width: 100%; max-width: 640px; max-height: 88vh;
  display: flex; flex-direction: column; gap: 0; overflow: hidden;
}
.modal-head, .modal-foot {
  display: flex; align-items: center; justify-content: space-between;
  padding: var(--sp-4) var(--sp-6);
}
.modal-head { border-bottom: 1px solid var(--border); }
.modal-foot { border-top: 1px solid var(--border); justify-content: flex-end; gap: var(--sp-2); }
.modal-head h2 { font-size: var(--fs-16); font-weight: var(--fw-semibold); }
.modal-body {
  padding: var(--sp-6);
  overflow-y: auto;
  display: flex; flex-direction: column; gap: var(--sp-4);
}
.row { display: flex; flex-direction: column; gap: 6px; }
.row.two { flex-direction: row; gap: var(--sp-4); }
.row.two > div { flex: 1; display: flex; flex-direction: column; gap: 6px; }
.row label { font-size: var(--fs-12); color: var(--text-muted); font-weight: var(--fw-medium); }
.opt-list { display: flex; flex-direction: column; gap: 6px; }
.opt-item { display: flex; gap: 6px; align-items: center; }
.icon-btn {
  width: 28px; height: 28px; border-radius: var(--radius-sm);
  color: var(--text-subtle); display: flex; align-items: center; justify-content: center;
}
.icon-btn:hover { color: var(--text); background: var(--surface-2); }
.error-msg { color: var(--danger); font-size: var(--fs-13); }
</style>
