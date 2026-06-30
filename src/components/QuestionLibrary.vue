<script setup lang="ts">
import { ref, onMounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";
import Icon from "./ui/Icon.vue";
import QuestionModal from "./ui/QuestionModal.vue";
import { useImportProgress } from "../composables/useImportProgress";
import { useToast } from "../composables/useToast";

interface Question {
  id: number;
  question_type: string;
  content: string;
  options: string | null;
  tags: string;
  difficulty: number;
  standard_answer: string;
  explanation: string;
  source: string;
  quality_status: string;
  quality_note: string;
  content_hash?: string;
  duplicate_of?: number | null;
}

const questions = ref<Question[]>([]);
const tags = ref<string[]>([]);
const tagCounts = ref<Record<string, number>>({});
const grandTotal = ref(0);        // 全库总数（不含筛选）
const filteredCount = ref(0);     // 当前筛选下的总数
const currentTag = ref<string>("全部");
const searchInput = ref("");      // 输入框实时值
const searchTerm = ref("");       // 防抖后真正发起查询的值
const pageSize = 30;
const offset = ref(0);
const loading = ref(false);
const newTopicName = ref("");
const creatingTopic = ref(false);
const topicMessage = ref("");

const { startImport } = useImportProgress();
const toast = useToast();

const modalMode = ref<"create" | "edit" | "view" | null>(null);
const modalQuestion = ref<Question | null>(null);

function openCreate() { modalQuestion.value = null; modalMode.value = "create"; }
function openEdit(q: Question) { modalQuestion.value = q; modalMode.value = "edit"; }
function openView(q: Question) { modalQuestion.value = q; modalMode.value = "view"; }
function closeModal() { modalMode.value = null; }
function onSaved() { modalMode.value = null; refresh(); }

async function handleExport() {
  const path = await saveDialog({
    defaultPath: "forgerust-questions.json",
    filters: [{ name: "JSON 题库", extensions: ["json"] }],
  });
  if (!path) return;
  try {
    const n = await invoke<number>("export_questions", { path });
    toast.success("导出完成", `已导出 ${n} 道题到 ${path}`);
  } catch (e) {
    toast.error("操作失败", String(e));
  }
}

async function handleMarkWrong(q: Question) {
  try {
    await invoke("mark_question_wrong", { questionId: q.id });
    toast.success("已加入错题本");
  } catch (e) {
    toast.error("操作失败", String(e));
  }
}

async function refresh() {
  loading.value = true;
  try {
    const tagArg = currentTag.value === "全部" ? null : currentTag.value;
    const searchArg = searchTerm.value.trim() || null;
    const [list, t, counts, fc, gc] = await Promise.all([
      invoke<Question[]>("list_questions", {
        tag: tagArg,
        search: searchArg,
        limit: pageSize,
        offset: offset.value,
      }),
      invoke<string[]>("get_all_tags"),
      invoke<Record<string, number>>("get_tag_counts"),
      invoke<number>("count_questions", { tag: tagArg, search: searchArg }),
      invoke<number>("count_questions", { tag: null, search: null }),
    ]);
    questions.value = list;
    tags.value = t;
    tagCounts.value = counts;
    filteredCount.value = fc;
    grandTotal.value = gc;
  } catch (e) {
    console.error(e);
    toast.error("题库加载失败", String(e));
  } finally {
    loading.value = false;
  }
}

onMounted(refresh);

function pickTag(tag: string) {
  currentTag.value = tag;
  offset.value = 0;
  refresh();
}

// 搜索防抖（300ms）
let searchTimer: ReturnType<typeof setTimeout> | null = null;
watch(searchInput, (v) => {
  if (searchTimer) clearTimeout(searchTimer);
  searchTimer = setTimeout(() => {
    searchTerm.value = v;
    offset.value = 0;
    refresh();
  }, 300);
});

function clearSearch() {
  searchInput.value = "";
  if (searchTimer) clearTimeout(searchTimer);
  searchTerm.value = "";
  offset.value = 0;
  refresh();
}

async function createTopic() {
  const name = newTopicName.value.trim();
  if (!name) return;
  creatingTopic.value = true;
  topicMessage.value = "";
  try {
    await invoke("create_topic", { name, description: null });
    newTopicName.value = "";
    topicMessage.value = "已创建";
    await refresh();
  } catch (e) {
    topicMessage.value = String(e);
  } finally {
    creatingTopic.value = false;
  }
}

async function nextPage() {
  if (offset.value + pageSize >= filteredCount.value) return;
  offset.value += pageSize;
  await refresh();
}
async function prevPage() {
  if (offset.value === 0) return;
  offset.value = Math.max(0, offset.value - pageSize);
  await refresh();
}

async function handleImport() {
  const selected = await open({
    multiple: false,
    filters: [{ name: "JSON 题库", extensions: ["json"] }],
  });
  if (!selected) return;
  const path = typeof selected === "string" ? selected : (selected as any).path;
  try {
    await startImport(path);
  } catch (e) {
    toast.error("操作失败", String(e));
  }
}

async function handleDelete(q: Question) {
  if (!confirm(`确定删除该题目？\n\n"${q.content.slice(0, 50)}..."`)) return;
  try {
    await invoke("delete_question", { id: q.id });
    await refresh();
  } catch (e) {
    toast.error("操作失败", String(e));
  }
}

function typeLabel(t: string): string {
  switch (t) {
    case "SINGLE": return "单选";
    case "MULTI":  return "多选";
    case "ESSAY":  return "简答";
    default:       return t;
  }
}
function difficultyLabel(d: number): string {
  return "★".repeat(Math.max(1, Math.min(5, d)));
}

function qualityLabel(status: string): string {
  switch (status) {
    case "reviewed": return "已审核";
    case "needs_review": return "需复核";
    case "outdated": return "可能过时";
    default: return "待审核";
  }
}
function qualityClass(status: string): string {
  if (status === "reviewed") return "ok";
  if (status === "needs_review") return "warn";
  if (status === "outdated") return "danger";
  return "muted";
}

</script>

<template>
  <div class="fr-page">
   <div class="library">
    <header class="head">
      <div>
        <h1 class="fr-page-title">题库管理</h1>
        <p class="fr-page-subtitle">
          浏览、搜索与导入题目。全库共 {{ grandTotal }} 题<span v-if="searchTerm || currentTag !== '全部'"> · 当前筛选 {{ filteredCount }} 题</span>。
        </p>
      </div>
      <div class="head-actions">
        <button class="fr-btn fr-btn-ghost" @click="handleExport">
          <Icon name="Download" :size="14" /><span>导出</span>
        </button>
        <button class="fr-btn fr-btn-ghost" @click="openCreate">
          <Icon name="Plus" :size="14" /><span>新增题目</span>
        </button>
        <button class="fr-btn fr-btn-primary" @click="handleImport">
          <Icon name="Upload" :size="14" /><span>导入题库</span>
        </button>
      </div>
    </header>

    <div class="search-box">
      <Icon name="Search" :size="14" class="search-icon" />
      <input
        v-model="searchInput"
        type="text"
        class="search-input"
        placeholder="搜索题干、标签或答案，例如：TCP"
      />
      <button v-if="searchInput" class="search-clear" title="清空搜索" @click="clearSearch">
        <Icon name="X" :size="12" />
      </button>
    </div>

    <div class="create-topic">
      <input
        v-model="newTopicName"
        class="fr-input create-topic-input"
        type="text"
        placeholder="新建考点，例如 Linux"
        @keyup.enter="createTopic"
      />
      <button class="fr-btn fr-btn-ghost" :disabled="creatingTopic || !newTopicName.trim()" @click="createTopic">
        <Icon name="Plus" :size="14" />
        <span>新建考点</span>
      </button>
      <span v-if="topicMessage" class="topic-msg">{{ topicMessage }}</span>
    </div>

    <div class="toolbar">
      <button
        :class="['tag-pill', { active: currentTag === '全部' }]"
        @click="pickTag('全部')"
      >
        全部 <span class="tag-count">{{ grandTotal }}</span>
      </button>
      <button
        v-for="t in tags"
        :key="t"
        :class="['tag-pill', { active: currentTag === t }]"
        @click="pickTag(t)"
      >
        {{ t }} <span class="tag-count">{{ tagCounts[t] ?? 0 }}</span>
      </button>
    </div>

    <div v-if="loading" class="state-msg">
      <Icon name="Loader2" :size="16" class="spin" />
      <span>加载中...</span>
    </div>

    <div v-else-if="questions.length === 0" class="state-msg">
      <p v-if="searchTerm">没有匹配「{{ searchTerm }}」的题目。</p>
      <p v-else>该筛选下暂无题目。</p>
    </div>

    <ul v-else class="q-list">
      <li v-for="q in questions" :key="q.id" class="q-row">
        <div class="q-meta">
          <span class="fr-chip">{{ typeLabel(q.question_type) }}</span>
          <span class="diff fr-mono" :title="`难度 ${q.difficulty}`">{{ difficultyLabel(q.difficulty) }}</span>
          <span :class="['quality-badge', qualityClass(q.quality_status)]" :title="q.quality_note || q.source || '暂无备注'">
            {{ qualityLabel(q.quality_status) }}
          </span>
          <span v-if="q.source" class="source-text">{{ q.source }}</span>
          <span v-if="q.duplicate_of" class="duplicate-badge" :title="q.quality_note || `与题目 #${q.duplicate_of} 疑似重复`">
            疑似重复 #{{ q.duplicate_of }}
          </span>
          <span class="q-tags">
            <span v-for="t in q.tags.split(',').map(s => s.trim()).filter(Boolean).slice(0, 3)" :key="t" class="fr-chip fr-chip-accent">{{ t }}</span>
          </span>
        </div>
        <div class="q-content">{{ q.content }}</div>
        <div class="q-actions">
          <button class="icon-btn" title="查看详情" @click="openView(q)">
            <Icon name="Eye" :size="14" />
          </button>
          <button class="icon-btn" title="编辑" @click="openEdit(q)">
            <Icon name="Pencil" :size="14" />
          </button>
          <button class="icon-btn" title="加入错题本" @click="handleMarkWrong(q)">
            <Icon name="BookmarkPlus" :size="14" />
          </button>
          <button class="icon-btn danger" title="删除" @click="handleDelete(q)">
            <Icon name="Trash2" :size="14" />
          </button>
        </div>
      </li>
    </ul>

    <div class="pager" v-if="!loading && filteredCount > pageSize">
      <button class="fr-btn fr-btn-ghost" :disabled="offset === 0" @click="prevPage">
        <Icon name="ChevronLeft" :size="14" /><span>上一页</span>
      </button>
      <span class="pager-info fr-mono">
        {{ offset + 1 }} – {{ Math.min(offset + pageSize, filteredCount) }} / {{ filteredCount }}
      </span>
      <button class="fr-btn fr-btn-ghost" :disabled="offset + pageSize >= filteredCount" @click="nextPage">
        <span>下一页</span><Icon name="ChevronRight" :size="14" />
      </button>
    </div>

    <QuestionModal
      v-if="modalMode"
      :mode="modalMode"
      :question="modalQuestion"
      :tags="tags"
      @close="closeModal"
      @saved="onSaved"
    />
   </div>
  </div>
</template>

<style scoped>
.library {
  width: 100%;
  max-width: 1200px;
  margin: 0 auto;
}

.head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--sp-4);
  margin-bottom: var(--sp-6);
}
.head-actions { display: flex; gap: var(--sp-2); align-items: center; }

.search-box {
  position: relative;
  display: flex;
  align-items: center;
  margin-bottom: var(--sp-4);
}
.search-icon {
  position: absolute;
  left: 12px;
  color: var(--text-subtle);
  pointer-events: none;
}
.search-input {
  width: 100%;
  padding: 10px 36px 10px 36px;
  font-size: var(--fs-13);
  color: var(--text);
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  outline: none;
  transition: border-color var(--dur-fast) var(--ease),
              box-shadow var(--dur-fast) var(--ease);
}
.search-input::placeholder { color: var(--text-subtle); }
.search-input:hover { border-color: var(--border-strong); }
.search-input:focus {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-soft);
}
.search-clear {
  position: absolute;
  right: 8px;
  width: 22px;
  height: 22px;
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-subtle);
  transition: all var(--dur-fast) var(--ease);
}
.search-clear:hover {
  color: var(--text);
  background: var(--surface-2);
}

.create-topic {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  margin-bottom: var(--sp-4);
}
.create-topic-input {
  max-width: 280px;
}
.topic-msg {
  font-size: var(--fs-12);
  color: var(--text-subtle);
}

.toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-bottom: var(--sp-4);
  padding-bottom: var(--sp-4);
  border-bottom: 1px solid var(--border);
}
.tag-pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
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
.tag-count {
  font-family: var(--font-mono);
  font-size: 11px;
  opacity: 0.7;
}

.state-msg {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--sp-2);
  padding: var(--sp-8);
  font-size: var(--fs-13);
  color: var(--text-muted);
}

.q-list {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  overflow: hidden;
}
.q-row {
  display: grid;
  /* minmax(0, 1fr) 关键 —— 默认的 1fr 等价 minmax(auto, 1fr)，会被长串中英文混排撑爆，
     导致整个题库管理区随题目内容变宽。显式给 0 作为下限即可强制收敛到容器宽度。 */
  grid-template-columns: auto minmax(0, 1fr) auto;
  gap: var(--sp-4);
  align-items: center;
  padding: var(--sp-3) var(--sp-6);
  border-bottom: 1px solid var(--border);
}
.q-row:last-child { border-bottom: none; }
.q-row:hover { background: var(--surface-2); }

.q-meta {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 100px;
}
.q-meta .diff {
  color: var(--warning);
  font-size: var(--fs-12);
  letter-spacing: 0.5px;
}
.quality-badge { width: fit-content; padding: 2px 7px; border-radius: 999px; font-size: 11px; line-height: 1.4; }
.quality-badge.ok { color: var(--success); background: var(--success-soft); }
.quality-badge.warn { color: var(--warning); background: var(--warning-soft); }
.quality-badge.danger { color: var(--danger); background: var(--danger-soft); }
.quality-badge.muted { color: var(--text-subtle); background: var(--surface-2); }
.duplicate-badge {
  width: fit-content;
  padding: 2px 7px;
  border-radius: 999px;
  color: var(--danger);
  background: var(--danger-soft);
  font-size: 11px;
  line-height: 1.4;
  white-space: nowrap;
}
.source-text { max-width: 120px; color: var(--text-subtle); font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.q-tags { display: flex; gap: 4px; flex-wrap: wrap; }

.q-content {
  min-width: 0;
  font-size: var(--fs-13);
  color: var(--text);
  line-height: 1.5;
  overflow: hidden;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  word-break: break-word;
  overflow-wrap: anywhere;
}

.icon-btn {
  width: 28px;
  height: 28px;
  border-radius: var(--radius-sm);
  color: var(--text-subtle);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all var(--dur-fast) var(--ease);
}
.icon-btn:hover { color: var(--text); background: var(--surface-2); }
.icon-btn.danger:hover { color: var(--danger); background: var(--danger-soft); }

.pager {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--sp-4);
  margin-top: var(--sp-6);
}
.pager-info {
  font-size: var(--fs-12);
  color: var(--text-muted);
}

.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
</style>

