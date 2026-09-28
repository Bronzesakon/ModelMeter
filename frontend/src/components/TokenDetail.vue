<template>
  <div class="glass-card p-5">
    <div class="flex items-center justify-between mb-3">
      <span class="text-sm font-semibold text-gray-700 dark:text-gray-300">今日 Token 详情</span>
      <div class="flex gap-1">
        <button
          v-for="m in models"
          :key="m.value"
          @click="store.selectedDetailModel = m.value"
          class="px-2 py-0.5 rounded-full text-[11px] font-medium transition"
          :class="store.selectedDetailModel === m.value
            ? 'bg-brand-500 text-white shadow-sm shadow-brand-500/20'
            : 'bg-white/80 dark:bg-zinc-600/70 text-gray-800 dark:text-gray-200 border border-gray-400/40 dark:border-white/20 shadow-sm'"
        >
          {{ m.label }}
        </button>
      </div>
    </div>

    <div class="grid grid-cols-2 gap-3 text-sm">
      <div>
        <div class="text-xs font-medium text-green-500 dark:text-green-500 mb-0.5">缓存命中</div>
        <div class="text-xl font-bold text-green-400 dark:text-green-400">{{ store.detailTodayCacheHit.toLocaleString() }}</div>
      </div>
      <div>
        <div class="text-xs font-medium text-orange-500 dark:text-orange-500 mb-0.5">缓存未命中</div>
        <div class="text-xl font-bold text-orange-400 dark:text-orange-400">{{ store.detailTodayCacheMiss.toLocaleString() }}</div>
      </div>
      <div>
        <div class="text-xs font-medium text-blue-500 dark:text-blue-500 mb-0.5">输出</div>
        <div class="text-xl font-bold text-blue-400 dark:text-blue-400">{{ store.detailTodayOutput.toLocaleString() }}</div>
      </div>
      <div>
        <div class="text-xs font-medium text-purple-500 dark:text-purple-500 mb-0.5">命中率</div>
        <div class="text-xl font-bold text-purple-400 dark:text-purple-400">{{ store.detailTodayCacheHitRate }}</div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useDashboardStore } from "@/stores/dashboard";
const store = useDashboardStore();

const models = [
  { value: "flash" as const, label: "V4.1 Flash" },
  { value: "pro" as const, label: "V4 Pro" },
];
</script>
