<script setup lang="ts">
import type { QuotaWindowCycleProgress } from './presenter'

defineProps<{
  progress: QuotaWindowCycleProgress
  label: string
}>()
</script>

<template>
  <div
    class="w-full overflow-hidden rounded-full bg-cp-border-secondary"
    role="progressbar"
    :aria-label="`${label}周期进度`"
    aria-valuemin="0"
    aria-valuemax="100"
    :aria-valuenow="Math.round(progress.elapsedPercent)"
    :aria-valuetext="`周期已过 ${progress.elapsedPercentDisplay}，距重置 ${progress.remainingDisplay}`"
    :title="`周期已过 ${progress.elapsedPercentDisplay} · 距重置 ${progress.remainingDisplay}`"
  >
    <div
      class="h-full rounded-full bg-cp-blue-solid transition-[width] duration-200 motion-reduce:transition-none"
      :style="progress.barStyle"
    />
  </div>
</template>
