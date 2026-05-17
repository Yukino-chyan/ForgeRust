<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Icon from "./ui/Icon.vue";

interface WrongQuestion {
  question_id: number;
  content: string;
  question_type: string;
  tags: string;
  difficulty: number;
  standard_answer: string;
  explanation: string;
  wrong_count: number;
  last_score: number;
  last_attempt: string;
  manually_added_count: number;
}

const props = defineProps<{ isActive?: boolean }>();
const emit = defineEmits<{ startWrongPractice: [ids: number[]] }>();

const list = ref<WrongQuestion[]>([]);
const isLoading = ref(false);
const filterTag = ref("");
const expandedId = ref<number | null>(null);

async function fetchList() {
  if (list.value.length === 0) isLoading.value = true;
  try {
    const data = await invoke<WrongQuestion[]>("get_wrong_questions");
    list.value = data;
  } catch (e) {
    console.error("加载错题本失败", e);
  } finally {
    isLoading.value = false;
  }
}

onMounted(fetchList);

watch(() => props.isActive, (active) => {
  if (active) fetchList();
});

const allTags = computed(() => {
  const set = new Set<string>();
  list.value.forEach((q) =>
    q.tags.split(",").forEach((t) => {
      const v = t.trim();
      if (v) set.add(v);
    })
  );
  return Array.from(set).sort();
});

const filtered = computed(() =>
  filterTag.value
    ? list.value.filter((q) => q.tags.includes(filterTag.value))
    : list.value
);

const typeLabel = (t: string) => ({ SINGLE: "单选", MULTI: "多选", ESSAY: "简答" }[t] ?? t);
const difficultyLabel = (d: number) => ["", "入门", "简单", "中等", "困难", "专家"][d] ?? "—";

function formatDate(s: string) {
  return s ? s.slice(0, 10) : "—";
}

async function removeQuestion(q: WrongQuestion, e: MouseEvent) {
  e.stopPropagation();
  try {
    await invoke("remove_from_wrong_book", { questionId: q.question_id });
    list.value = list.value.filter((item) => item.question_id !== q.question_id);
    if (expandedId.value === q.question_id) expandedId.value = null;
  } catch (err) {
    console.error("删除失败", err);
  }
}

function startPractice() {
  const ids = filtered.value.map((q) => q.question_id);
  emit("startWrongPractice", ids);
}
</script>

<template>
  <div class="fr-page wb">
    <header class="head">
      <div>
        <h1 class="fr-page-title">错题本</h1>
        <p class="fr-page-subtitle">
          共 {{ filtered.length }} 道错题{{ filterTag ? ` · ${filterTag}` : "" }}
        </p>
      </div>
      <button class="fr-btn fr-btn-primary" :disabled="filtered.length === 0" @click="startPractice">
        <Icon name="Play" :size="14" />
        <span>重练错题</span>
      </button>
    </header>

    <div class="filter-bar">
      <button
        :class="['tag-pill', { active: filterTag === '' }]"
        @click="filterTag = ''"
      >
        全部
      </button>
      <button
        v-for="tag in allTags"
        :key="tag"
        :class="['tag-pill', { active: filterTag === tag }]"
        @click="filterTag = tag"
      >
        {{ tag }}
      </button>
    </div>

    <div v-if="isLoading" class="state">
      <Icon name="Loader2" :size="16" class="spin" />
      <span>加载中...</span>
    </div>

    <div v-else-if="filtered.length === 0" class="empty">
      <Icon name="CheckCircle2" :size="40" :stroke-width="1.5" />
      <p>{{ filterTag ? "该标签下暂无错题" : "暂无错题，继续保持。" }}</p>
    </div>

    <div v-else class="list">
      <div
        v-for="q in filtered"
        :key="q.question_id"
        :class="['item', { open: expandedId === q.question_id }]"
        @click="expandedId = expandedId === q.question_id ? null : q.question_id"
      >
        <div class="row">
          <div class="row-left">
            <span class="wrong-count fr-mono">×{{ q.wrong_count }}</span>
          </div>

          <div class="row-body">
            <p class="q-content">{{ q.content }}</p>
            <div class="q-meta">
              <span class="fr-chip">{{ typeLabel(q.question_type) }}</span>
              <span class="fr-chip">{{ difficultyLabel(q.difficulty) }}</span>
              <span v-for="t in q.tags.split(',').map(s => s.trim()).filter(Boolean).slice(0, 3)" :key="t" class="fr-chip fr-chip-accent">{{ t }}</span>
              <span v-if="q.manually_added_count > 0" class="fr-chip pin">
                <Icon name="Pin" :size="10" /> 手动标记
              </span>
            </div>
          </div>

          <div class="row-right">
            <span :class="['score', 'fr-mono', q.last_score >= 60 ? 'score-ok' : 'score-bad']">
              {{ q.last_score }}
            </span>
            <span class="date fr-mono">{{ formatDate(q.last_attempt) }}</span>
            <button class="icon-btn" title="从错题本移除" @click="removeQuestion(q, $event)">
              <Icon name="X" :size="14" />
            </button>
          </div>
        </div>

        <div class="answer-wrap">
          <div class="answer">
            <div class="answer-inner">
              <div class="answer-label">标准答案</div>
              <p class="answer-text">{{ q.standard_answer }}</p>
              <div class="answer-label explanation">解析</div>
              <p class="answer-text">{{ q.explanation }}</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.wb { max-width: var(--content-max); margin: 0 auto; }

.head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--sp-4);
  margin-bottom: var(--sp-6);
}

.filter-bar {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-bottom: var(--sp-4);
  padding-bottom: var(--sp-4);
  border-bottom: 1px solid var(--border);
}
.tag-pill {
  padding: 6px 12px;
  border-radius: 999px;
  font-size: var(--fs-12);
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

.state {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--sp-2);
  padding: var(--sp-8);
  color: var(--text-muted);
  font-size: var(--fs-13);
}
.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--sp-3);
  padding: var(--sp-12) 0;
  color: var(--text-subtle);
}
.empty p { font-size: var(--fs-13); }

.list {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
}

.item {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  overflow: hidden;
  cursor: pointer;
  transition: border-color var(--dur-fast) var(--ease);
}
.item:hover { border-color: var(--border-strong); }
.item.open { border-color: var(--accent); }

.row {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  padding: var(--sp-3) var(--sp-4);
}

.row-left { flex-shrink: 0; }
.wrong-count {
  display: inline-block;
  padding: 2px 8px;
  border-radius: var(--radius-sm);
  background: var(--danger-soft);
  color: var(--danger);
  font-size: var(--fs-12);
  font-weight: var(--fw-semibold);
}

.row-body { flex: 1; min-width: 0; }
.q-content {
  font-size: var(--fs-13);
  color: var(--text);
  line-height: 1.5;
  margin-bottom: 6px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.q-meta { display: flex; gap: 4px; flex-wrap: wrap; }
.fr-chip.pin {
  background: var(--warning-soft);
  color: var(--warning);
  border-color: transparent;
}

.row-right {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 4px;
  flex-shrink: 0;
}
.score {
  font-size: var(--fs-20);
  font-weight: var(--fw-semibold);
  line-height: 1;
}
.score-ok  { color: var(--success); }
.score-bad { color: var(--danger); }
.date {
  font-size: 11px;
  color: var(--text-subtle);
}

.icon-btn {
  width: 24px;
  height: 24px;
  border-radius: var(--radius-sm);
  color: var(--text-subtle);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all var(--dur-fast) var(--ease);
}
.icon-btn:hover {
  color: var(--danger);
  background: var(--danger-soft);
}

.answer-wrap {
  display: grid;
  grid-template-rows: 0fr;
  transition: grid-template-rows var(--dur-base) var(--ease);
}
.item.open .answer-wrap { grid-template-rows: 1fr; }
.answer { overflow: hidden; min-height: 0; }
.answer-inner {
  max-height: 220px;
  overflow-y: auto;
  padding: var(--sp-4);
  border-top: 1px solid var(--border);
  background: var(--surface-2);
}
.answer-label {
  font-size: 11px;
  font-weight: var(--fw-semibold);
  color: var(--success);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin-bottom: 4px;
}
.answer-label.explanation {
  color: var(--info);
  margin-top: var(--sp-3);
}
.answer-text {
  font-size: var(--fs-13);
  color: var(--text);
  line-height: 1.6;
}

.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
</style>
