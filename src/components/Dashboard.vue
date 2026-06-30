<script setup lang="ts">
import { ref, onMounted, watch, computed, shallowRef, onBeforeUnmount, nextTick } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Icon from "./ui/Icon.vue";
import { useToast } from "../composables/useToast";

import * as echarts from "echarts/core";
import { LineChart, BarChart } from "echarts/charts";
import {
  GridComponent,
  TooltipComponent,
  TitleComponent,
} from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
echarts.use([LineChart, BarChart, GridComponent, TooltipComponent, TitleComponent, CanvasRenderer]);

const props = defineProps<{ isActive: boolean }>();
const emit = defineEmits<{
  (e: "start-training", payload: { mode: "wrong" | "weak" | "random"; ids?: number[] }): void;
  (e: "navigate", view: string): void;
}>();

interface DashboardStats {
  total_answered: number;
  overall_accuracy: number;
  mastered_tags: number;
  total_tags: number;
  pending_review: number;
  streak_days: number;
  today_answered: number;
  week_delta_answered: number;
  week_delta_accuracy: number;
}
interface DayPoint { date: string; accuracy: number; count: number; }
interface TagStat { tag: string; accuracy: number; total: number; }
interface SessionRecord { id: number; started_at: string; total: number; correct: number; tags: string[]; }

const stats = ref<DashboardStats | null>(null);
const trend = ref<DayPoint[]>([]);
const mastery = ref<TagStat[]>([]);
const sessions = ref<SessionRecord[]>([]);
const loading = ref(true);
const toast = useToast();

const trendEl = ref<HTMLDivElement | null>(null);
const masteryEl = ref<HTMLDivElement | null>(null);
const trendChart = shallowRef<echarts.ECharts | null>(null);
const masteryChart = shallowRef<echarts.ECharts | null>(null);

const greeting = computed(() => {
  const h = new Date().getHours();
  if (h < 11) return "早上好，开发者";
  if (h < 13) return "中午好，开发者";
  if (h < 18) return "下午好，开发者";
  return "晚上好，开发者";
});

const hasData = computed(() => (stats.value?.total_answered ?? 0) > 0);
const accuracyPct = computed(() =>
  stats.value ? (stats.value.overall_accuracy * 100).toFixed(1) : "—"
);
const weakestTag = computed(() => {
  const eligible = mastery.value.filter((t) => t.total >= 3);
  if (eligible.length === 0) return null;
  return [...eligible].sort((a, b) => a.accuracy - b.accuracy)[0];
});

function cssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || "#000";
}

async function fetchAll() {
  loading.value = true;
  try {
    const [s, t, m, sess] = await Promise.all([
      invoke<DashboardStats>("get_dashboard_stats"),
      invoke<DayPoint[]>("get_accuracy_trend", { days: 30 }),
      invoke<TagStat[]>("get_tag_mastery"),
      invoke<SessionRecord[]>("get_recent_sessions", { limit: 5 }),
    ]);
    stats.value = s;
    trend.value = t;
    mastery.value = m;
    sessions.value = sess;
  } catch (e) {
    console.error("Dashboard 数据加载失败", e);
  } finally {
    loading.value = false;
  }
}

function renderTrend() {
  if (!trendEl.value) return;
  if (!trendChart.value) trendChart.value = echarts.init(trendEl.value);
  const accent = cssVar("--accent");
  const accentSoft = cssVar("--accent-soft");
  const textMuted = cssVar("--text-muted");
  const textSubtle = cssVar("--text-subtle");
  const border = cssVar("--border");

  trendChart.value.setOption({
    grid: { left: 40, right: 12, top: 16, bottom: 28 },
    tooltip: {
      trigger: "axis",
      backgroundColor: cssVar("--surface"),
      borderColor: cssVar("--border"),
      textStyle: { color: cssVar("--text"), fontSize: 12 },
      formatter: (params: any) => {
        const p = params[0];
        const point = trend.value.find((d) => d.date === p.axisValue);
        if (!point) return "";
        return `${point.date}<br/>${point.count} 题 · ${(point.accuracy * 100).toFixed(1)}%`;
      },
    },
    xAxis: {
      type: "category",
      data: trend.value.map((d) => d.date.slice(5)),
      axisLine: { lineStyle: { color: border } },
      axisLabel: { color: textSubtle, fontSize: 11 },
      axisTick: { show: false },
    },
    yAxis: {
      type: "value",
      min: 0,
      max: 100,
      splitLine: { lineStyle: { color: border, type: "dashed" } },
      axisLabel: { color: textSubtle, fontSize: 11, formatter: "{value}%" },
      axisLine: { show: false },
      axisTick: { show: false },
    },
    series: [
      {
        type: "line",
        data: trend.value.map((d) => +(d.accuracy * 100).toFixed(1)),
        smooth: true,
        symbol: "circle",
        symbolSize: 6,
        lineStyle: { width: 2, color: accent },
        itemStyle: { color: accent, borderColor: cssVar("--surface"), borderWidth: 2 },
        areaStyle: { color: accentSoft },
      },
    ],
    textStyle: { color: textMuted },
  });
  trendChart.value.resize();
}

function renderMastery() {
  if (!masteryEl.value) return;
  if (!masteryChart.value) masteryChart.value = echarts.init(masteryEl.value);
  const data = mastery.value.filter((t) => t.total >= 5).slice(0, 8).reverse();
  const success = cssVar("--success");
  const warning = cssVar("--warning");
  const danger = cssVar("--danger");
  const border = cssVar("--border");
  const textMuted = cssVar("--text-muted");
  const textSubtle = cssVar("--text-subtle");

  masteryChart.value.setOption({
    grid: { left: 90, right: 40, top: 8, bottom: 24 },
    tooltip: {
      trigger: "axis",
      backgroundColor: cssVar("--surface"),
      borderColor: cssVar("--border"),
      textStyle: { color: cssVar("--text"), fontSize: 12 },
      formatter: (params: any) => {
        const p = params[0];
        const item = data.find((d) => d.tag === p.name);
        if (!item) return "";
        return `${item.tag}<br/>${(item.accuracy * 100).toFixed(1)}% (${Math.round(item.accuracy * item.total)}/${item.total})`;
      },
    },
    xAxis: {
      type: "value",
      min: 0,
      max: 100,
      splitLine: { lineStyle: { color: border, type: "dashed" } },
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: { color: textSubtle, fontSize: 11, formatter: "{value}%" },
    },
    yAxis: {
      type: "category",
      data: data.map((d) => d.tag),
      axisLine: { lineStyle: { color: border } },
      axisTick: { show: false },
      axisLabel: { color: textMuted, fontSize: 12 },
    },
    series: [
      {
        type: "bar",
        data: data.map((d) => ({
          value: +(d.accuracy * 100).toFixed(1),
          itemStyle: {
            color: d.accuracy >= 0.8 ? success : d.accuracy >= 0.5 ? warning : danger,
            borderRadius: [0, 4, 4, 0],
          },
        })),
        barWidth: 14,
        label: {
          show: true,
          position: "right",
          color: textMuted,
          fontSize: 11,
          formatter: "{c}%",
        },
      },
    ],
  });
  masteryChart.value.resize();
}

watch(() => props.isActive, async (active) => {
  if (active) {
    await fetchAll();
    await nextTick();
    if (hasData.value) {
      renderTrend();
      renderMastery();
    }
  }
});

onMounted(async () => {
  if (props.isActive) {
    await fetchAll();
    await nextTick();
    if (hasData.value) {
      renderTrend();
      renderMastery();
    }
  }
  window.addEventListener("resize", handleResize);
});
onBeforeUnmount(() => {
  window.removeEventListener("resize", handleResize);
  trendChart.value?.dispose();
  masteryChart.value?.dispose();
});
function handleResize() {
  trendChart.value?.resize();
  masteryChart.value?.resize();
}

async function actionWrong() {
  try {
    const wrong = await invoke<any[]>("get_wrong_questions");
    const ids = wrong.map((w) => w.question_id);
    if (ids.length === 0) {
      emit("navigate", "wrong_book");
      return;
    }
    emit("start-training", { mode: "wrong", ids });
  } catch (e) {
    console.error(e);
  }
}
function actionWeak() {
  emit("start-training", { mode: "weak" });
}
function actionRandom() {
  emit("start-training", { mode: "random" });
}

function formatDate(s: string): string {
  return s.slice(0, 10);
}
function sessionAccuracy(r: SessionRecord): string {
  if (!r.total) return "—";
  return ((r.correct / r.total) * 100).toFixed(0) + "%";
}

const pendingDelete = ref<SessionRecord | null>(null);
const deleting = ref(false);

function askDelete(r: SessionRecord) {
  pendingDelete.value = r;
}
function cancelDelete() {
  pendingDelete.value = null;
}
async function confirmDelete() {
  if (!pendingDelete.value) return;
  deleting.value = true;
  try {
    await invoke("delete_session", { id: pendingDelete.value.id });
    pendingDelete.value = null;
    await fetchAll();
    await nextTick();
    if (hasData.value) {
      renderTrend();
      renderMastery();
    }
  } catch (e) {
    toast.error("删除失败", String(e));
  } finally {
    deleting.value = false;
  }
}
</script>

<template>
  <div class="fr-page dashboard">
    <header class="head">
      <div>
        <h1 class="fr-page-title">{{ greeting }}</h1>
        <p class="fr-page-subtitle">查看你的训练数据与进度。</p>
      </div>
      <div v-if="stats" class="streak-chip">
        <Icon name="Flame" :size="14" />
        <span v-if="stats.today_answered > 0">
          今日已练 <strong class="fr-mono">{{ stats.today_answered }}</strong> 题 · 连续
          <strong class="fr-mono">{{ stats.streak_days }}</strong> 天
        </span>
        <span v-else class="streak-cta" @click="emit('navigate', 'training')">
          今天还没开始，来一题？
        </span>
      </div>
    </header>

    <!-- KPI 卡片 -->
    <section class="kpi-grid">
      <div class="fr-card kpi">
        <div class="kpi-label">累计做题</div>
        <div class="kpi-value fr-mono">{{ stats?.total_answered ?? "—" }}</div>
        <div v-if="stats && stats.week_delta_answered !== 0" class="kpi-delta" :class="stats.week_delta_answered > 0 ? 'up' : 'down'">
          <Icon :name="stats.week_delta_answered > 0 ? 'TrendingUp' : 'TrendingDown'" :size="12" />
          <span>{{ stats.week_delta_answered > 0 ? "+" : "" }}{{ stats.week_delta_answered }} 本周</span>
        </div>
      </div>

      <div class="fr-card kpi">
        <div class="kpi-label">整体正确率</div>
        <div class="kpi-value fr-mono">{{ hasData ? accuracyPct + "%" : "—" }}</div>
        <div v-if="stats && Math.abs(stats.week_delta_accuracy) >= 0.1" class="kpi-delta" :class="stats.week_delta_accuracy > 0 ? 'up' : 'down'">
          <Icon :name="stats.week_delta_accuracy > 0 ? 'TrendingUp' : 'TrendingDown'" :size="12" />
          <span>{{ stats.week_delta_accuracy > 0 ? "+" : "" }}{{ stats.week_delta_accuracy.toFixed(1) }}pp</span>
        </div>
      </div>

      <div class="fr-card kpi">
        <div class="kpi-label">已掌握标签</div>
        <div class="kpi-value fr-mono">
          {{ stats?.mastered_tags ?? "—" }}
          <span class="kpi-sub">/ {{ stats?.total_tags ?? "—" }}</span>
        </div>
        <div class="kpi-hint">正确率 ≥ 80% 且 ≥ 5 题</div>
      </div>

      <div class="fr-card kpi">
        <div class="kpi-label">待复习</div>
        <div class="kpi-value fr-mono">{{ stats?.pending_review ?? "—" }}</div>
        <div class="kpi-hint">来自错题本</div>
      </div>
    </section>

    <!-- 图表 -->
    <section class="charts-grid">
      <div class="fr-card chart-card">
        <div class="card-head">
          <h3 class="card-title">正确率趋势</h3>
          <span class="card-sub">近 30 天</span>
        </div>
        <div v-if="hasData" ref="trendEl" class="chart"></div>
        <div v-else class="empty-chart">
          <Icon name="LineChart" :size="32" :stroke-width="1.5" />
          <p>暂无数据</p>
        </div>
      </div>

      <div class="fr-card chart-card">
        <div class="card-head">
          <h3 class="card-title">标签掌握度</h3>
          <span class="card-sub">题数 ≥ 5 的标签</span>
        </div>
        <div v-if="hasData && mastery.filter(t => t.total >= 5).length > 0" ref="masteryEl" class="chart"></div>
        <div v-else class="empty-chart">
          <Icon name="BarChart3" :size="32" :stroke-width="1.5" />
          <p>暂无数据</p>
        </div>
      </div>
    </section>

    <!-- 行动卡 -->
    <section class="action-section">
      <h3 class="section-title">继续训练</h3>
      <div class="action-grid">
        <button
          class="action-card"
          :disabled="(stats?.pending_review ?? 0) === 0"
          @click="actionWrong"
        >
          <div class="action-icon"><Icon name="Bookmark" :size="20" /></div>
          <div class="action-body">
            <div class="action-title">错题再练</div>
            <div class="action-sub">
              {{ stats?.pending_review ?? 0 }} 题待复习
            </div>
          </div>
          <Icon name="ArrowRight" :size="16" class="action-arrow" />
        </button>

        <button
          class="action-card"
          :disabled="!weakestTag"
          @click="actionWeak"
        >
          <div class="action-icon"><Icon name="Target" :size="20" /></div>
          <div class="action-body">
            <div class="action-title">薄弱标签</div>
            <div class="action-sub">
              <template v-if="weakestTag">
                {{ weakestTag.tag }} · {{ (weakestTag.accuracy * 100).toFixed(0) }}%
              </template>
              <template v-else>数据不足</template>
            </div>
          </div>
          <Icon name="ArrowRight" :size="16" class="action-arrow" />
        </button>

        <button class="action-card" @click="actionRandom">
          <div class="action-icon"><Icon name="Shuffle" :size="20" /></div>
          <div class="action-body">
            <div class="action-title">随机练习</div>
            <div class="action-sub">跨标签快速练习</div>
          </div>
          <Icon name="ArrowRight" :size="16" class="action-arrow" />
        </button>
      </div>
    </section>

    <!-- 最近会话 -->
    <section class="recent-section">
      <h3 class="section-title">最近会话</h3>
      <div v-if="sessions.length === 0" class="empty-list">
        <p>暂无会话记录。</p>
      </div>
      <ul v-else class="session-list">
        <li v-for="s in sessions" :key="s.id" class="session-row">
          <div class="session-date fr-mono">{{ formatDate(s.started_at) }}</div>
          <div class="session-meta">
            <span class="fr-mono">{{ s.total }} 题</span>
            <span class="dot">·</span>
            <span class="fr-mono">{{ sessionAccuracy(s) }}</span>
          </div>
          <div class="session-tags">
            <span v-for="t in s.tags.slice(0, 3)" :key="t" class="fr-chip">{{ t }}</span>
            <span v-if="s.tags.length > 3" class="session-tag-more">+{{ s.tags.length - 3 }}</span>
          </div>
          <div class="session-action">
            <button
              class="session-del"
              title="删除该会话（同时清除其答题记录）"
              @click="askDelete(s)"
            >
              <Icon name="Trash2" :size="14" />
            </button>
          </div>
        </li>
      </ul>
    </section>

    <!-- 删除确认弹窗 -->
    <Transition name="modal">
      <div v-if="pendingDelete" class="modal-backdrop" @click.self="cancelDelete">
        <div class="modal">
          <div class="modal-icon"><Icon name="AlertTriangle" :size="20" /></div>
          <h3 class="modal-title">删除这次会话？</h3>
          <p class="modal-body">
            {{ formatDate(pendingDelete.started_at) }} · {{ pendingDelete.total }} 题 ·
            正确率 {{ sessionAccuracy(pendingDelete) }}
            <br />
            <span class="modal-warn">
              该会话的所有答题记录会一并删除，Dashboard 上的累计数据与趋势会随之更新。此操作不可撤销。
            </span>
          </p>
          <div class="modal-actions">
            <button class="fr-btn fr-btn-ghost" :disabled="deleting" @click="cancelDelete">取消</button>
            <button class="fr-btn fr-btn-danger" :disabled="deleting" @click="confirmDelete">
              {{ deleting ? "删除中..." : "确定删除" }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.dashboard { max-width: var(--content-max); margin: 0 auto; }

.head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--sp-4);
  margin-bottom: var(--sp-6);
}
.streak-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 8px 14px;
  border-radius: 999px;
  background: var(--accent-soft);
  color: var(--accent);
  font-size: var(--fs-13);
  font-weight: var(--fw-medium);
}
.streak-chip strong { font-weight: var(--fw-semibold); margin: 0 2px; }
.streak-cta { cursor: pointer; }

/* KPI */
.kpi-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: var(--sp-4);
  margin-bottom: var(--sp-6);
}
.kpi { padding: var(--sp-4) var(--sp-6); }
.kpi-label {
  font-size: var(--fs-12);
  color: var(--text-muted);
  margin-bottom: var(--sp-2);
}
.kpi-value {
  font-size: var(--fs-28);
  font-weight: var(--fw-semibold);
  color: var(--text);
  line-height: 1.1;
}
.kpi-sub {
  font-size: var(--fs-14);
  color: var(--text-subtle);
  font-weight: var(--fw-regular);
  margin-left: 2px;
}
.kpi-delta {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  margin-top: var(--sp-2);
  font-size: var(--fs-12);
  font-weight: var(--fw-medium);
}
.kpi-delta.up   { color: var(--success); }
.kpi-delta.down { color: var(--danger); }
.kpi-hint {
  margin-top: var(--sp-2);
  font-size: var(--fs-12);
  color: var(--text-subtle);
}

/* Charts */
.charts-grid {
  display: grid;
  grid-template-columns: 1.4fr 1fr;
  gap: var(--sp-4);
  margin-bottom: var(--sp-6);
}
.chart-card { padding: var(--sp-4) var(--sp-6); }
.card-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  margin-bottom: var(--sp-3);
}
.card-title {
  font-size: var(--fs-14);
  font-weight: var(--fw-semibold);
  color: var(--text);
}
.card-sub {
  font-size: var(--fs-12);
  color: var(--text-subtle);
}
.chart { width: 100%; height: 240px; }
.empty-chart {
  height: 240px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--sp-2);
  color: var(--text-subtle);
  font-size: var(--fs-13);
}

/* Sections */
.section-title {
  font-size: var(--fs-14);
  font-weight: var(--fw-semibold);
  color: var(--text);
  margin-bottom: var(--sp-3);
}

/* Action cards */
.action-section { margin-bottom: var(--sp-6); }
.action-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: var(--sp-4);
}
.action-card {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  padding: var(--sp-4) var(--sp-6);
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  text-align: left;
  transition: border-color var(--dur-fast) var(--ease),
              transform var(--dur-fast) var(--ease);
  box-shadow: var(--shadow-sm);
}
.action-card:hover:not(:disabled) {
  border-color: var(--accent);
  transform: translateY(-1px);
}
.action-card:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.action-icon {
  width: 36px;
  height: 36px;
  border-radius: var(--radius-md);
  background: var(--accent-soft);
  color: var(--accent);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.action-body { flex: 1; min-width: 0; }
.action-title {
  font-size: var(--fs-14);
  font-weight: var(--fw-medium);
  color: var(--text);
}
.action-sub {
  font-size: var(--fs-12);
  color: var(--text-muted);
  margin-top: 2px;
}
.action-arrow {
  color: var(--text-subtle);
  flex-shrink: 0;
}
.action-card:hover:not(:disabled) .action-arrow { color: var(--accent); }

/* Recent sessions */
.recent-section { margin-bottom: var(--sp-12); }
.empty-list {
  padding: var(--sp-8);
  text-align: center;
  color: var(--text-subtle);
  font-size: var(--fs-13);
  background: var(--surface);
  border: 1px dashed var(--border);
  border-radius: var(--radius-lg);
}
.session-list {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  overflow: hidden;
}
.session-row {
  display: grid;
  grid-template-columns: 110px 130px 1fr 80px;
  align-items: center;
  padding: 12px var(--sp-6);
  border-bottom: 1px solid var(--border);
  font-size: var(--fs-13);
  gap: var(--sp-4);
}
.session-row:last-child { border-bottom: none; }
.session-date { color: var(--text-muted); }
.session-meta { display: flex; gap: 6px; align-items: center; color: var(--text); }
.session-meta .dot { color: var(--text-subtle); }
.session-tags { display: flex; gap: 4px; flex-wrap: wrap; min-width: 0; }
.session-tag-more {
  font-size: var(--fs-12);
  color: var(--text-subtle);
  padding: 3px 6px;
}
.session-action { text-align: right; }
.session-del {
  width: 28px;
  height: 28px;
  border-radius: var(--radius-sm);
  color: var(--text-subtle);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  transition: all var(--dur-fast) var(--ease);
}
.session-del:hover {
  color: var(--danger);
  background: var(--danger-soft);
}

/* Modal */
.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 50;
}
.modal {
  width: 400px;
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
  background: var(--danger-soft);
  color: var(--danger);
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
  line-height: 1.6;
}
.modal-warn {
  display: inline-block;
  margin-top: 8px;
  color: var(--text-subtle);
  font-size: var(--fs-12);
}
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--sp-2);
}
.modal-enter-active, .modal-leave-active { transition: opacity var(--dur-base) var(--ease); }
.modal-enter-from, .modal-leave-to { opacity: 0; }
</style>

