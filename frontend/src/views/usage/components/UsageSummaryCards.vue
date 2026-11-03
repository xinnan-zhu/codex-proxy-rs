<script setup lang="ts">
import type { getUsageRecordSummary, UsageCostMetric, UsageMetricResponse } from '@/api'
import { ZCard, ZMotionIcon } from '@codex-proxy/ui'

import { Activity, CircleDollarSign, Database, FileText, Timer } from '@lucide/vue'
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { decimalDisplayNumber, formatUsd } from '@/utils/format'

const props = defineProps<{
  summary: Omit<Awaited<ReturnType<typeof getUsageRecordSummary>>, 'analytics'> | null
  error?: string
  cost: UsageMetricResponse<UsageCostMetric>['data']['summary'] | null
}>()

const { t } = useI18n({ useScope: 'global' })

const averageLatency = computed(() => {
  const raw = props.summary?.averageLatencyMsValue
  const value = raw == null || !Number.isFinite(raw) || raw < 0
    ? 0
    : raw
  if (value < 1_000) {
    return {
      value: String(Math.round(value)),
      unit: 'ms',
    }
  }
  if (value < 60_000) {
    return {
      value: (value / 1_000).toFixed(2),
      unit: 's',
    }
  }
  if (value < 3_600_000) {
    return {
      value: (value / 60_000).toFixed(1),
      unit: 'min',
    }
  }
  return {
    value: (value / 3_600_000).toFixed(1),
    unit: 'h',
  }
})

const costDetail = computed(() => {
  const estimatedCost = decimalDisplayNumber(props.cost?.estimatedCost)
  const millionsPerDollar = estimatedCost != null && estimatedCost > 0 && (props.cost?.billedTotalTokens ?? 0) > 0
    ? (props.cost?.billedTotalTokens ?? 0) / estimatedCost / 1_000_000
    : null
  return millionsPerDollar != null && Number.isFinite(millionsPerDollar)
    ? t('usage.fork.efficiency', {
        tokens: millionsPerDollar.toFixed(2),
      })
    : t('usage.fork.efficiency', { tokens: '— ' })
})

const items = computed(() => [
  {
    key: 'requests',
    label: t('usage.successfulRequest'),
    icon: Activity,
    value: props.summary?.totalRequests,
    detail: t('usage.withinFilterRange'),
    tone: 'bg-z-blue-container text-z-blue-on-container',
  },
  {
    key: 'cost',
    label: t('usage.billing.totalCost'),
    icon: CircleDollarSign,
    value: formatUsd(props.cost?.estimatedCost),
    detail: costDetail.value,
    tone: 'bg-z-purple-container text-z-purple-on-container',
  },
  {
    key: 'tokens',
    label: t('usage.tokens.totalTokens'),
    icon: FileText,
    value: props.summary?.totalTokens,
    detail: t('usage.inputOutput', { inputTokens: props.summary?.inputTokens ?? '0', outputTokens: props.summary?.outputTokens ?? '0' }),
    tone: 'bg-z-green-container text-z-green-on-container',
  },
  {
    key: 'cached',
    label: t('usage.cachedTokens'),
    icon: Database,
    value: props.summary?.cachedInputTokens,
    detail: t('usage.cacheReadHit'),
    tone: 'bg-z-orange-container text-z-orange-on-container',
  },
  {
    key: 'latency',
    label: t('usage.averageTimeTaken'),
    icon: Timer,
    value: averageLatency.value.value,
    unit: averageLatency.value.unit,
    detail: t('usage.successfulRequestAverage'),
    tone: 'bg-z-cyan-container text-z-cyan-on-container',
  },
])
</script>

<template>
  <section class="mt-5 grid shrink-0 grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-5" :aria-label="t('usage.usageOverview')">
    <ZCard
      v-for="item in items"
      :key="item.key"
      as="article"
      padding="compact"
      class="grid min-h-23 grid-cols-[36px_minmax(0,1fr)] items-stretch gap-3"
    >
      <ZMotionIcon class="inline-flex size-9 shrink-0 items-center justify-center rounded-z" :class="item.tone">
        <component :is="item.icon" class="size-4.5" />
      </ZMotionIcon>
      <div class="flex min-w-0 flex-col">
        <span class="block text-z-sm leading-none font-heavy text-z-text-secondary">
          {{ item.label }}
        </span>
        <strong class="my-2 block truncate font-mono text-[24px] leading-none font-extrabold text-z-text">
          {{ item.value ?? '0' }}<span v-if="item.unit" class="ml-0.5 font-sans text-z font-semibold text-z-text-secondary">{{ item.unit }}</span>
        </strong>
        <span class="block truncate text-z-sm leading-none font-emphasis text-z-text-quaternary" :title="error || item.detail">
          {{ error || item.detail }}
        </span>
      </div>
    </ZCard>
  </section>
</template>
