<script setup lang="ts">
import { ref, watch, shallowRef, nextTick, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Icon from "./ui/Icon.vue";

import * as echarts from "echarts/core";
import { RadarChart } from "echarts/charts";
import { TooltipComponent } from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
echarts.use([RadarChart, TooltipComponent, CanvasRenderer]);

interface DimensionScores { project_depth: number; fundamental_solidity: number; communication: number; }
interface InterviewSummary { id: number; created_at: string; candidate: string; tags: string; average_score: number; dimension_scores: DimensionScores; }
interface InterviewReport {
  interview_id: number;
  average_score: number;
  dimension_scores: DimensionScores;
  summary: string;
  messages: { role: string; phase: string; content: string; seq: number }[];
}

const props = defineProps<{ isActive: boolean }>();

const view = ref<"list" | "detail">("list");
const loading = ref(false);
const errorMsg = ref("");
const items = ref<InterviewSummary[]>([]);
const confirmingId = ref<number | null>(null);

const report = ref<InterviewReport | null>(null);
const radarEl = ref<HTMLElement | null>(null);
const radarChart = shallowRef<echarts.ECharts | null>(null);

watch(
  () => props.isActive,
  (active) => {
    if (active && view.value === "list") loadList();
  },
  { immediate: true }
);

async function loadList() {
  loading.value = true;
  errorMsg.value = "";
  try {
    items.value = await invoke<InterviewSummary[]>("list_interviews");
  } catch (err) {
    errorMsg.value = String(err);
  } finally {
    loading.value = false;
  }
}

async function openDetail(id: number) {
  loading.value = true;
  errorMsg.value = "";
  try {
    report.value = await invoke<InterviewReport>("get_interview_detail", { interviewId: id });
    view.value = "detail";
    nextTick(renderRadar);
  } catch (err) {
    errorMsg.value = String(err);
  } finally {
    loading.value = false;
  }
}

function backToList() {
  view.value = "list";
  radarChart.value?.dispose();
  radarChart.value = null;
  report.value = null;
}

function requestDelete(id: number) { confirmingId.value = id; }
function cancelDelete() { confirmingId.value = null; }
async function confirmDelete(id: number) {
  try {
    await invoke("delete_interview", { interviewId: id });
    items.value = items.value.filter((it) => it.id !== id);
  } catch (err) {
    errorMsg.value = String(err);
  } finally {
    confirmingId.value = null;
  }
}

function renderRadar() {
  if (!radarEl.value || !report.value) return;
  if (!radarChart.value) radarChart.value = echarts.init(radarEl.value);
  const d = report.value.dimension_scores;
  radarChart.value.setOption({
    tooltip: {},
    radar: {
      indicator: [
        { name: "项目深度", max: 100 },
        { name: "八股扎实度", max: 100 },
        { name: "表达逻辑", max: 100 },
      ],
      radius: "65%",
    },
    series: [{
      type: "radar",
      data: [{ value: [d.project_depth, d.fundamental_solidity, d.communication], name: "本场表现" }],
      areaStyle: { opacity: 0.2 },
    }],
  });
}

onUnmounted(() => { radarChart.value?.dispose(); });
</script>

<template>
  <div class="fr-page history-page">
    <header class="hist-head">
      <div>
        <h1 class="fr-page-title">面试记录</h1>
        <p class="fr-page-subtitle">回顾已完成的模拟面试与多维复盘。</p>
      </div>
      <button v-if="view === 'detail'" class="fr-btn fr-btn-ghost" @click="backToList">
        <Icon name="ArrowLeft" :size="14" /><span>返回列表</span>
      </button>
    </header>

    <!-- 列表态 -->
    <section v-if="view === 'list'" class="list-panel">
      <p v-if="loading" class="empty-hint">加载中…</p>
      <p v-else-if="!items.length" class="empty-hint">还没有已完成的面试记录，去「模拟面试」开始一场吧。</p>
      <ul v-else class="hist-list">
        <li v-for="it in items" :key="it.id" class="hist-row fr-card">
          <div class="row-main">
            <span class="row-candidate">{{ it.candidate || "未命名候选人" }}</span>
            <span v-if="it.tags" class="row-tags">{{ it.tags }}</span>
            <span class="row-date">{{ it.created_at }}</span>
          </div>
          <span class="row-score">{{ it.average_score.toFixed(1) }}</span>
          <div class="row-actions">
            <template v-if="confirmingId === it.id">
              <span class="confirm-text">确定删除？</span>
              <button class="fr-btn fr-btn-ghost" @click="cancelDelete">取消</button>
              <button class="fr-btn fr-btn-danger" @click="confirmDelete(it.id)">删除</button>
            </template>
            <template v-else>
              <button class="fr-btn fr-btn-ghost" @click="openDetail(it.id)"><Icon name="Eye" :size="14" /><span>查看</span></button>
              <button class="icon-btn danger" @click="requestDelete(it.id)"><Icon name="Trash2" :size="15" /></button>
            </template>
          </div>
        </li>
      </ul>
      <p v-if="errorMsg" class="error-msg">{{ errorMsg }}</p>
    </section>

    <!-- 详情态 -->
    <section v-else-if="report" class="detail-panel">
      <div class="fr-card report-hero">
        <div>
          <span class="report-label">面试复盘</span>
          <h2>{{ report.average_score.toFixed(1) }}</h2>
          <p>{{ report.summary }}</p>
        </div>
      </div>

      <div class="fr-card dim-card">
        <h3>能力维度</h3>
        <div ref="radarEl" class="radar"></div>
      </div>

      <div class="transcript-replay">
        <div v-for="(m, i) in report.messages" :key="i" :class="['bubble-row', m.role === 'interviewer' ? 'interviewer' : 'candidate']">
          <div class="bubble">{{ m.content }}</div>
        </div>
      </div>
      <p v-if="errorMsg" class="error-msg">{{ errorMsg }}</p>
    </section>
  </div>
</template>

<style scoped>
.history-page { max-width: var(--content-max); margin: 0 auto; display: flex; flex-direction: column; gap: var(--sp-4); }
.hist-head { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--sp-4); }
.list-panel, .detail-panel { display: flex; flex-direction: column; gap: var(--sp-3); }
.empty-hint { font-size: var(--fs-13); color: var(--text-subtle); padding: var(--sp-6); text-align: center; }
.error-msg { color: var(--danger); font-size: var(--fs-13); }

.hist-list { display: flex; flex-direction: column; gap: var(--sp-2); }
.hist-row { display: flex; align-items: center; gap: var(--sp-4); padding: var(--sp-3) var(--sp-4); }
.row-main { flex: 1; display: flex; align-items: center; gap: var(--sp-3); min-width: 0; }
.row-candidate { font-size: var(--fs-14); font-weight: var(--fw-medium); color: var(--text); }
.row-tags { font-size: var(--fs-12); color: var(--accent); background: var(--accent-soft); border-radius: 999px; padding: 2px 8px; }
.row-date { font-size: var(--fs-12); color: var(--text-subtle); }
.row-score { font-size: var(--fs-20); font-family: var(--font-mono); color: var(--accent); }
.row-actions { display: flex; align-items: center; gap: var(--sp-2); }
.confirm-text { font-size: var(--fs-12); color: var(--text-muted); }
.icon-btn { width: 30px; height: 30px; border-radius: var(--radius-sm); color: var(--text-subtle); display: inline-flex; align-items: center; justify-content: center; }
.icon-btn.danger:hover { color: var(--danger); background: var(--danger-soft); }

.report-hero { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--sp-4); }
.report-label { font-size: var(--fs-12); color: var(--text-muted); }
.report-hero h2 { font-size: var(--fs-28); color: var(--accent); margin: var(--sp-1) 0; }
.report-hero p { color: var(--text-muted); line-height: 1.6; }
.dim-card { display: flex; flex-direction: column; gap: var(--sp-3); }
.dim-card h3 { font-size: var(--fs-14); font-weight: var(--fw-semibold); }
.radar { width: 100%; height: 280px; }

.transcript-replay { display: flex; flex-direction: column; gap: var(--sp-2); }
.bubble-row { display: flex; }
.bubble-row.interviewer { justify-content: flex-start; }
.bubble-row.candidate { justify-content: flex-end; }
.bubble { max-width: 78%; padding: 8px 12px; font-size: var(--fs-13); line-height: 1.6; border-radius: var(--radius-lg); }
.bubble-row.interviewer .bubble { background: var(--accent-soft); color: var(--text); border-bottom-left-radius: var(--radius-sm); }
.bubble-row.candidate .bubble { background: var(--surface-2); color: var(--text); border: 1px solid var(--border); border-bottom-right-radius: var(--radius-sm); }
</style>
