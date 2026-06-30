<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, shallowRef, watch } from "vue";
import * as echarts from "echarts/core";
import { RadarChart } from "echarts/charts";
import { TooltipComponent } from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";

echarts.use([RadarChart, TooltipComponent, CanvasRenderer]);

interface DimensionScores {
  project_depth: number;
  fundamental_solidity: number;
  communication: number;
}

const props = defineProps<{ scores: DimensionScores }>();
const radarEl = ref<HTMLElement | null>(null);
const chart = shallowRef<echarts.ECharts | null>(null);

function renderRadar() {
  if (!radarEl.value) return;
  if (!chart.value) chart.value = echarts.init(radarEl.value);
  chart.value.setOption({
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
      data: [{
        value: [props.scores.project_depth, props.scores.fundamental_solidity, props.scores.communication],
        name: "本场表现",
      }],
      areaStyle: { opacity: 0.2 },
    }],
  });
}

onMounted(() => nextTick(renderRadar));
watch(() => props.scores, () => nextTick(renderRadar), { deep: true });
onUnmounted(() => chart.value?.dispose());
</script>

<template>
  <div ref="radarEl" class="score-radar"></div>
</template>

<style scoped>
.score-radar { width: 100%; height: 280px; }
</style>
