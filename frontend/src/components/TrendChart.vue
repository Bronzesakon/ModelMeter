<template>
  <div class="glass-card px-5 pt-5 pb-3">
    <div class="flex items-center justify-between mb-3">
      <span class="text-sm font-semibold text-gray-700 dark:text-gray-300">7 日趋势</span>
      <div class="flex gap-1">
        <button
          v-for="m in models"
          :key="m.value"
          @click="store.selectedTrendModel = m.value"
          class="px-2 py-0.5 rounded-full text-[11px] font-medium transition"
          :class="store.selectedTrendModel === m.value
            ? 'bg-brand-500 text-white shadow-sm shadow-brand-500/20'
            : 'bg-white/80 dark:bg-zinc-600/70 text-gray-800 dark:text-gray-200 border border-gray-400/40 dark:border-white/20 shadow-sm'"
        >
          {{ m.label }}
        </button>
      </div>
    </div>

    <div v-if="store.trendChartData.length === 0" class="text-center py-8 text-gray-400 dark:text-gray-500 text-sm">
      暂无用量数据
    </div>
    <div v-else ref="chartWrap" class="relative" style="height: 130px" @mouseleave="hoveredIdx = null">
      <Bar :data="chartData" :options="chartOptions" />
      <BarTooltip
        v-if="hoveredIdx !== null && store.trendChartData[hoveredIdx]"
        :visible="true"
        :x="tooltipX"
        :y="tooltipY"
        :data="store.trendChartData[hoveredIdx]"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";
import { Bar } from "vue-chartjs";
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  BarElement,
  Tooltip,
} from "chart.js";
import { invoke } from "@tauri-apps/api/core";
import { useDashboardStore } from "@/stores/dashboard";
import { useThemeStore } from "@/stores/theme";
import BarTooltip from "@/components/BarTooltip.vue";

ChartJS.register(CategoryScale, LinearScale, BarElement, Tooltip);

const store = useDashboardStore();
const theme = useThemeStore();

const models = [
  { value: "flash" as const, label: "V4.1 Flash" },
  { value: "pro" as const, label: "V4 Pro" },
];

const hoveredIdx = ref<number | null>(null);
const tooltipX = ref(0);
const tooltipY = ref(0);
const chartWrap = ref<HTMLElement>();

function showDetail(idx: number) {
  const point = store.trendChartData[idx];
  if (!point) return;
  const total = point.cacheHit + point.cacheMiss;
  const rate = total > 0 ? `${((point.cacheHit / total) * 100).toFixed(2)}%` : "--";
  invoke("ds_toggle_trend_detail", {
    date: point.date,
    cacheHit: point.cacheHit,
    cacheMiss: point.cacheMiss,
    output: point.output,
    cacheHitRate: rate,
    cost: point.cost,
    audioDuration: point.audioDuration,
  });
}

const chartData = computed(() => ({
  labels: store.trendChartData.map((p) => p.label),
  datasets: [
    {
      data: store.trendChartData.map((p) => p.tokens),
      backgroundColor: (ctx: any) => {
        const gradient = ctx.chart.ctx.createLinearGradient(0, 0, 0, 130);
        gradient.addColorStop(0, "#818CF8");
        gradient.addColorStop(1, "#4F6EF7");
        return gradient;
      },
      borderRadius: document.body.dataset.winVersion === "10" ? 0 : 4,
      borderSkipped: false,
      maxBarThickness: 32,
    },
  ],
}));

const chartOptions = computed(() => ({
  responsive: true,
  maintainAspectRatio: false,
  layout: {
    padding: { bottom: 2 },
  },
  plugins: {
    tooltip: {
      enabled: false,
      external: (context: any) => {
        const tooltipModel = context.tooltip;
        if (tooltipModel.opacity === 0 || tooltipModel.dataPoints?.length === 0) {
          hoveredIdx.value = null;
          return;
        }
        const idx = tooltipModel.dataPoints[0].dataIndex;
        hoveredIdx.value = idx;
        const caretX = tooltipModel.caretX;
        const caretY = tooltipModel.caretY;
        const chartRect = context.chart.canvas.getBoundingClientRect();
        tooltipX.value = caretX + chartRect.left;
        tooltipY.value = caretY + chartRect.top;
      },
    },
  },
  onHover: (_event: any, elements: any[]) => {
    if (elements.length === 0) {
      hoveredIdx.value = null;
    }
  },
  onClick: (_event: any, elements: any[], chart: any) => {
    if (elements.length > 0) {
      showDetail(elements[0].index);
      return;
    }
    const rect = chart.canvas.getBoundingClientRect();
    const canvasX = (_event.native?.clientX ?? _event.x ?? 0) - rect.left;
    const meta = chart.getDatasetMeta(0);
    for (let i = 0; i < meta.data.length; i++) {
      const bar = meta.data[i];
      if (canvasX >= bar.x - bar.width / 2 && canvasX <= bar.x + bar.width / 2) {
        showDetail(i);
        return;
      }
    }
  },
  scales: {
    x: {
      grid: { display: false },
      ticks: { color: "#9CA3AF", font: { size: 11 } },
    },
    y: {
      grid: { color: theme.isDark ? "rgba(255,255,255,0.06)" : "rgba(0,0,0,0.05)" },
      ticks: {
        color: "#9CA3AF",
        font: { size: 11 },
        callback: (v: any) => v >= 1000 ? `${(v / 1000).toFixed(0)}K` : v,
      },
    },
  },
}));
</script>
