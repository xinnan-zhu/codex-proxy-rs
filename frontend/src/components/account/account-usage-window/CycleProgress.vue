<script setup lang="ts">
import type { QuotaWindowCycleProgress } from './presenter'
import { useI18n } from 'vue-i18n'

defineProps<{
  progress: QuotaWindowCycleProgress
  label: string
}>()

const { t } = useI18n({ useScope: 'global' })
</script>

<template>
  <div
    class="w-full overflow-hidden rounded-full bg-cp-border-secondary"
    role="progressbar"
    :aria-label="t('accounts.fork.cycleLabel', { label })"
    aria-valuemin="0"
    aria-valuemax="100"
    :aria-valuenow="Math.round(progress.elapsedPercent)"
    :aria-valuetext="t('accounts.fork.cycleText', { percent: progress.elapsedPercentDisplay, remaining: progress.remainingDisplay })"
    :title="t('accounts.fork.cycleText', { percent: progress.elapsedPercentDisplay, remaining: progress.remainingDisplay })"
  >
    <div
      class="h-full rounded-full bg-cp-blue-solid transition-[width] duration-200 motion-reduce:transition-none"
      :style="progress.barStyle"
    />
  </div>
</template>
