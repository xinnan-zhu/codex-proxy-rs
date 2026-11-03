<script setup lang="ts">
import type { AccountQuotaWindow } from '@/api'
import { ZEmpty } from '@codex-proxy/ui'
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useUiClock } from '@/composables/useUiClock'
import AccountRequestTimeline from './account-usage-window/AccountRequestTimeline.vue'
import AccountQuotaCycleProgress from './account-usage-window/CycleProgress.vue'
import { quotaWindowCost, quotaWindowCycleProgress, quotaWindowDetails, quotaWindowLabel, resolveAccountUsageWindowPresentation } from './account-usage-window/presenter'

const props = withDefaults(defineProps<{
  label: string | null
  windows: AccountQuotaWindow[]
  showConsumption?: boolean
}>(), { showConsumption: true })
const { t } = useI18n({ useScope: 'global' })
const now = useUiClock()
const detailHeading = computed(() => props.label)
const detailTitle = computed(() => props.label ?? t('accounts.quota.quotaDetails'))
const detailItems = computed(() =>
  props.windows.map((window) => {
    const view = resolveAccountUsageWindowPresentation({
      window,
      variant: 'detail',
      showLocalValue: true,
    })

    return {
      ...quotaWindowDetails(window),
      cost: quotaWindowCost(window),
      cycle: quotaWindowCycleProgress(window, now.value.getTime()),
      key: window.key,
      local: view.mode === 'local',
      localLabel: view.local.label,
      requestDisplay: view.local.requestDisplay,
      requestBars: view.local.requestBars,
      timelineTitle: view.local.timelineTitle,
      durationDisplay: view.local.durationDisplay,
      code: quotaWindowCode(window.windowSeconds, window.role),
      label: quotaWindowLabel(window, false),
      usedPercent: window.usedPercent,
      percentTextClass: view.quota.percentTextClass,
      barClass: view.quota.barClass,
      barStyle: view.quota.barStyle,
    }
  }),
)
function quotaWindowCode(windowSeconds: number | null, role: AccountQuotaWindow['role']) {
  if (typeof windowSeconds === 'number' && Number.isFinite(windowSeconds)) {
    const daySeconds = 24 * 60 * 60
    const hourSeconds = 60 * 60

    if (windowSeconds >= daySeconds && windowSeconds % daySeconds === 0)
      return `${windowSeconds / daySeconds}D`
    if (windowSeconds >= hourSeconds && windowSeconds % hourSeconds === 0)
      return `${windowSeconds / hourSeconds}H`
  }

  switch (role) {
    case 'primary':
      return 'P'
    case 'secondary':
      return 'S'
    case 'monthly':
      return 'M'
    default:
      return 'Q'
  }
}
</script>

<template>
  <section class="w-84 max-w-[calc(100vw-2rem)] overflow-hidden rounded-z-lg" role="dialog" :aria-label="t('accounts.quota.details', { detailTitle })">
    <header v-if="detailHeading" class="bg-z-popover-header-bg px-3 py-2.5">
      <h3 class="m-0 truncate text-z leading-5 font-heavy text-z-text">
        {{ detailHeading }}
      </h3>
    </header>

    <ZEmpty v-if="detailItems.length === 0" :title="t('accounts.quota.unobserved')" size="small" surface="none" />
    <div v-else class="grid gap-4" :class="detailHeading ? 'px-3 pt-3 pb-3' : 'px-4 pt-4 pb-3'">
      <article v-for="item in detailItems" :key="item.key">
        <div class="grid grid-cols-[32px_minmax(0,1fr)_auto] items-center gap-2.5">
          <span
            class="grid h-5 w-8 place-items-center rounded-sm bg-z-blue-container-strong font-mono text-[9px] leading-none font-heavy tracking-[0.04em] whitespace-nowrap text-z-blue-on-container"
          >
            {{ item.code }}
          </span>
          <h4 class="m-0 min-w-0 truncate text-z-sm leading-4 font-heavy text-z-text-secondary">
            {{ item.local ? item.localLabel : item.label }}
          </h4>
          <strong
            class="font-mono text-z-xs leading-none font-heavy tabular-nums"
            :class="item.local ? 'text-z-text' : item.percentTextClass"
          >
            {{ item.local ? t('accounts.quota.requestCount', { requestDisplay: item.requestDisplay }) : item.quotaValueDisplay }}
          </strong>
        </div>

        <AccountRequestTimeline
          v-if="item.local"
          class="mt-2 h-1 w-full"
          :bars="item.requestBars"
          :label="item.timelineTitle"
        />
        <template v-else>
          <div
            class="mt-2 h-1 w-full overflow-hidden rounded-full bg-z-border-secondary"
            role="progressbar"
            :aria-label="item.label"
            aria-valuemin="0"
            aria-valuemax="100"
            :aria-valuenow="item.stale ? undefined : item.usedPercent ?? undefined"
            :aria-valuetext="item.stale ? `${item.quotaValueDisplay} · ${item.observationDisplay}` : item.quotaValueDisplay"
          >
            <div
              class="h-full rounded-full transition-[width,background-color] duration-200 motion-reduce:transition-none"
              :class="item.barClass"
              :style="item.barStyle"
            />
          </div>
          <AccountQuotaCycleProgress v-if="item.cycle" class="mt-1 h-1" :progress="item.cycle" :label="item.label" />
          <p v-if="item.cycle" class="mt-1 flex justify-between text-[10px] text-z-text-tertiary">
            <span>{{ t('accounts.fork.untilReset') }}</span>
            <strong class="font-mono text-z-blue-text">{{ item.cycle.remainingDisplay }}</strong>
          </p>
        </template>

        <dl class="mt-2.5 grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1.5 text-[10px] leading-4">
          <template v-if="item.local">
            <dt class="font-emphasis text-z-text-tertiary">
              {{ t('accounts.quota.statisticalMethod') }}
            </dt>
            <dd class="m-0 text-right font-mono font-heavy tabular-nums text-z-text">
              {{ t('accounts.quota.rollingWindow') }}
            </dd>
            <dt class="font-emphasis text-z-text-tertiary">
              {{ t('accounts.quota.statisticalRange') }}
            </dt>
            <dd class="m-0 truncate text-right font-mono font-emphasis tabular-nums text-z-text-secondary">
              {{ item.durationDisplay }}
            </dd>
          </template>
          <template v-else>
            <template v-if="showConsumption && item.consumptionDisplay">
              <dt class="font-emphasis text-z-text-tertiary">
                {{ t('accounts.quota.windowConsumption') }}
              </dt>
              <dd class="m-0 text-right font-mono font-heavy tabular-nums text-z-text">
                {{ item.consumptionDisplay }}
              </dd>
            </template>
            <template v-if="item.cost">
              <dt class="font-emphasis text-z-text-tertiary">
                {{ t('accounts.fork.periodCost') }}
              </dt>
              <dd class="m-0 text-right font-mono text-z-green-text" :title="item.cost.usedPartial ? t('accounts.fork.partialCost') : undefined">
                {{ item.cost.usedDisplay }}{{ item.cost.usedPartial ? '+' : '' }}
              </dd>
              <dt class="font-emphasis text-z-text-tertiary">
                {{ t('accounts.fork.estimatedQuota') }}
              </dt>
              <dd class="m-0 text-right font-mono text-z-text" :title="t('accounts.fork.estimateHelp')">
                ≈ {{ item.cost.estimatedTotalDisplay }}
              </dd>
            </template>
            <dt class="font-emphasis text-z-text-tertiary">
              {{ t('accounts.quota.resetTime') }}
            </dt>
            <dd class="m-0 truncate text-right font-mono font-emphasis tabular-nums text-z-text-secondary">
              {{ item.resetAtDisplay }}
            </dd>
          </template>
        </dl>
      </article>
    </div>
  </section>
</template>
