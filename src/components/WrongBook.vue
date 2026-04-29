<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";

interface WrongQuestion {
  question_id: number;
  content: string;
  question_type: string;
  tags: string;
  difficulty: number;
  standard_answer: string;
  wrong_count: number;
  last_score: number;
  last_attempt: string;
  manually_added_count: number;
}

const emit = defineEmits<{ startWrongPractice: [ids: number[]] }>();

const list = ref<WrongQuestion[]>([]);
const isLoading = ref(true);
const filterTag = ref('');
const expandedId = ref<number | null>(null);

onMounted(async () => {
  try {
    list.value = await invoke<WrongQuestion[]>("get_wrong_questions");
  } catch (e) {
    console.error("加载错题本失败", e);
  } finally {
    isLoading.value = false;
  }
});

const allTags = computed(() => {
  const set = new Set<string>();
  list.value.forEach(q => q.tags.split(',').forEach(t => set.add(t.trim())));
  return Array.from(set).sort();
});

const filtered = computed(() =>
  filterTag.value
    ? list.value.filter(q => q.tags.includes(filterTag.value))
    : list.value
);

const typeLabel = (t: string) => ({ SINGLE: '单选', MULTI: '多选', ESSAY: '简答' }[t] ?? t);
const difficultyLabel = (d: number) => ['', '入门', '简单', '中等', '困难', '专家'][d] ?? '未知';

function formatDate(s: string) {
  return s ? s.slice(0, 10) : '--';
}

function startPractice() {
  const ids = filtered.value.map(q => q.question_id);
  emit('startWrongPractice', ids);
}
</script>

<template>
  <div class="wrongbook-container">
    <div class="wb-header">
      <div>
        <h2 class="wb-title">错题本</h2>
        <p class="wb-subtitle">共 {{ filtered.length }} 道错题{{ filterTag ? `（${filterTag}）` : '' }}</p>
      </div>
      <button
        class="practice-btn"
        :disabled="filtered.length === 0"
        @click="startPractice"
      >重练错题 →</button>
    </div>

    <!-- 标签筛选 -->
    <div class="tag-filter">
      <button
        :class="['filter-tag', { active: filterTag === '' }]"
        @click="filterTag = ''"
      >全部</button>
      <button
        v-for="tag in allTags" :key="tag"
        :class="['filter-tag', { active: filterTag === tag }]"
        @click="filterTag = tag"
      >{{ tag }}</button>
    </div>

    <!-- 加载中 -->
    <div v-if="isLoading" class="wb-empty">加载中...</div>

    <!-- 空状态 -->
    <div v-else-if="filtered.length === 0" class="wb-empty">
      <div class="empty-icon">🎉</div>
      <p>{{ filterTag ? '该考点暂无错题' : '暂无错题记录，继续加油！' }}</p>
    </div>

    <!-- 错题列表 -->
    <div v-else class="wb-list">
      <div
        v-for="q in filtered" :key="q.question_id"
        class="wb-item"
        @click="expandedId = expandedId === q.question_id ? null : q.question_id"
      >
        <div class="wb-item-main">
          <div class="wb-item-left">
            <span class="wrong-count">×{{ q.wrong_count }}</span>
          </div>
          <div class="wb-item-body">
            <p class="wb-question">{{ q.content }}</p>
            <div class="wb-meta">
              <span class="itag">{{ q.tags }}</span>
              <span class="itag">{{ typeLabel(q.question_type) }}</span>
              <span class="itag">{{ difficultyLabel(q.difficulty) }}</span>
              <span v-if="q.manually_added_count > 0" class="itag itag-marked">📌 手动标记</span>
            </div>
          </div>
          <div class="wb-item-right">
            <span :class="['last-score', q.last_score >= 60 ? 'score-ok' : 'score-bad']">
              {{ q.last_score }}<span class="score-unit">分</span>
            </span>
            <span class="last-date">{{ formatDate(q.last_attempt) }}</span>
          </div>
        </div>

        <!-- 展开：标准答案 -->
        <Transition name="expand">
          <div v-if="expandedId === q.question_id" class="wb-answer">
            <span class="answer-label">标准答案</span>
            <p>{{ q.standard_answer }}</p>
          </div>
        </Transition>
      </div>
    </div>
  </div>
</template>

<style scoped>
.wrongbook-container {
  height: 100%;
  overflow-y: auto;
  padding: 28px 32px 48px;
  max-width: 1040px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.wb-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}
.wb-title {
  font-size: 1.6rem;
  font-weight: 700;
  background: linear-gradient(90deg, #4facfe, #00d4ff);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  margin-bottom: 4px;
}
.wb-subtitle { font-size: 0.875rem; color: #4a5568; }

.practice-btn {
  padding: 10px 22px;
  border: none;
  border-radius: 10px;
  background: linear-gradient(135deg, #4facfe, #00d4ff);
  color: #080d18;
  font-size: 0.9rem;
  font-weight: 700;
  cursor: pointer;
  white-space: nowrap;
  transition: all 0.2s;
  box-shadow: 0 4px 16px rgba(79,172,254,0.3);
  flex-shrink: 0;
}
.practice-btn:hover:not(:disabled) { transform: translateY(-1px); box-shadow: 0 6px 24px rgba(79,172,254,0.45); }
.practice-btn:disabled { background: rgba(255,255,255,0.08); color: #4a5568; cursor: not-allowed; box-shadow: none; }

.tag-filter { display: flex; flex-wrap: wrap; gap: 6px; }
.filter-tag {
  padding: 5px 14px;
  border-radius: 7px;
  border: 1px solid rgba(99,179,237,0.15);
  background: transparent;
  color: #4a5568;
  font-size: 0.8rem;
  cursor: pointer;
  transition: all 0.18s;
}
.filter-tag:hover { border-color: rgba(79,172,254,0.35); color: #90cdf4; }
.filter-tag.active { background: rgba(79,172,254,0.12); border-color: rgba(79,172,254,0.4); color: #4facfe; font-weight: 600; }

.wb-empty { text-align: center; padding: 60px 0; color: #4a5568; }
.empty-icon { font-size: 3rem; margin-bottom: 12px; }

.wb-list { display: flex; flex-direction: column; gap: 8px; }

.wb-item {
  background: rgba(13,21,41,0.6);
  border: 1px solid rgba(99,179,237,0.1);
  border-radius: 12px;
  overflow: hidden;
  cursor: pointer;
  transition: border-color 0.18s;
}
.wb-item:hover { border-color: rgba(79,172,254,0.25); }

.wb-item-main {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 14px 16px;
}

.wb-item-left { min-width: 36px; text-align: center; }
.wrong-count {
  font-size: 0.82rem;
  font-weight: 700;
  color: #fc8181;
  background: rgba(252,129,129,0.1);
  border: 1px solid rgba(252,129,129,0.2);
  border-radius: 6px;
  padding: 2px 7px;
}

.wb-item-body { flex: 1; min-width: 0; }
.wb-question {
  font-size: 0.9rem;
  color: #e2e8f0;
  margin-bottom: 6px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.wb-meta { display: flex; gap: 6px; flex-wrap: wrap; }
.itag {
  font-size: 0.68rem;
  padding: 2px 8px;
  border-radius: 4px;
  background: rgba(99,179,237,0.08);
  color: #4a5568;
  border: 1px solid rgba(99,179,237,0.12);
}
.itag-marked { background: rgba(246,173,85,0.1); color: #f6ad55; border-color: rgba(246,173,85,0.2); }

.wb-item-right { display: flex; flex-direction: column; align-items: flex-end; gap: 4px; min-width: 52px; }
.last-score { font-size: 1.3rem; font-weight: 800; line-height: 1; }
.score-ok  { color: #68d391; }
.score-bad { color: #fc8181; }
.score-unit { font-size: 0.7rem; font-weight: 400; }
.last-date { font-size: 0.68rem; color: #4a5568; }

.wb-answer {
  padding: 12px 16px;
  border-top: 1px solid rgba(99,179,237,0.1);
  background: rgba(104,211,145,0.05);
}
.answer-label {
  font-size: 0.68rem;
  font-weight: 600;
  color: #68d391;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  display: block;
  margin-bottom: 6px;
}
.wb-answer p { font-size: 0.85rem; color: #9ae6b4; line-height: 1.6; }

.expand-enter-active, .expand-leave-active { transition: all 0.2s ease; }
.expand-enter-from, .expand-leave-to { opacity: 0; transform: translateY(-6px); }
</style>
