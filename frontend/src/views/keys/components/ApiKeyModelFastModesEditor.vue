<script setup lang="ts">
import type { ApiKeyFastModel, ModelFastMode, ModelFastModes } from '@/api/modules/keys'
import { ZButton, ZEmpty, ZInput, ZScrollbar, ZSegmented } from '@codex-proxy/ui'
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { getApiKeyFastModels } from '@/api/modules/keys'
import { useRequestState } from '@/composables/useRequestState'

const props = defineProps<{ active: boolean, disabled: boolean }>()
const model = defineModel<ModelFastModes>({ required: true })
const { t } = useI18n({ useScope: 'global' })
const catalog = ref<ApiKeyFastModel[]>([])
const search = ref('')
const request = useRequestState()
const { loading, error } = request
const models = computed(() => {
  const entries = new Map(catalog.value.map(item => [
    JSON.stringify([item.provider, item.model]),
    item,
  ]))
  for (const [provider, rules] of Object.entries(model.value)) {
    for (const id of Object.keys(rules)) {
      const key = JSON.stringify([provider, id])
      if (!entries.has(key)) {
        entries.set(key, {
          provider,
          model: id,
          label: id,
          supportsPriority: false,
        })
      }
    }
  }
  const query = search.value.trim().toLowerCase()
  return [
    ...entries.values(),
  ].filter(item => `${item.provider} ${item.model} ${item.label}`.toLowerCase().includes(query))
})

function options(item: ApiKeyFastModel) {
  return [
    {
      label: t('keys.editor.inheritFastMode'),
      value: 'default',
    },
    {
      label: t('common.status.enabled'),
      value: 'enabled',
      disabled: !item.supportsPriority,
    },
    {
      label: t('common.status.disabled'),
      value: 'disabled',
    },
  ]
}

function select(item: ApiKeyFastModel, mode: string) {
  if (props.disabled || (mode === 'enabled' && !item.supportsPriority))
    return
  const rules = { ...model.value[item.provider] }
  if (mode === 'default')
    delete rules[item.model]
  else rules[item.model] = mode as ModelFastMode
  const next = { ...model.value }
  if (Object.keys(rules).length)
    next[item.provider] = rules
  else delete next[item.provider]
  model.value = next
}

async function load() {
  const id = request.start()
  try {
    const result = await getApiKeyFastModels({ signal: request.signal, silent: true })
    if (request.isCurrent(id))
      catalog.value = result
  }
  catch (cause) {
    request.fail(id, cause)
  }
  finally {
    request.finish(id)
  }
}

watch(() => props.active, (active) => {
  request.invalidate()
  catalog.value = []
  search.value = ''
  error.value = ''
  if (active)
    void load()
}, { immediate: true })
</script>

<template>
  <div class="grid min-w-0 gap-3">
    <p class="text-sm text-cp-text-secondary">
      {{ t('keys.editor.modelFastModeHint') }}
    </p>
    <ZInput v-model="search" :aria-label="t('keys.editor.searchFastModels')" :placeholder="t('keys.editor.searchFastModels')" :disabled="disabled" />
    <div v-if="error" class="flex flex-wrap items-center gap-2 text-sm text-cp-text-secondary">
      <span>{{ error }}</span>
      <ZButton size="small" :disabled="disabled" @click="load">
        {{ t('common.actions.retry') }}
      </ZButton>
    </div>
    <ZScrollbar max-height="16rem">
      <div class="grid gap-3">
        <div v-for="item in models" :key="JSON.stringify([item.provider, item.model])" class="flex min-w-0 flex-wrap items-center justify-between gap-2">
          <div class="min-w-0 flex-1 basis-40">
            <div class="break-all text-sm">
              {{ item.label }}
            </div>
            <div class="break-all text-xs text-cp-text-secondary">
              {{ item.provider }} · {{ item.model }}
            </div>
            <p v-if="!item.supportsPriority" class="text-xs text-cp-text-secondary">
              {{ t('keys.editor.fastUnavailable') }}
            </p>
          </div>
          <ZSegmented
            :model-value="model[item.provider]?.[item.model] ?? 'default'"
            :options="options(item)"
            :aria-label="t('keys.editor.modelFastModeFor', { model: item.model })"
            :disabled="disabled"
            class="w-60 max-w-full"
            @update:model-value="select(item, $event)"
          />
        </div>
        <ZEmpty v-if="!models.length" :description="loading ? t('common.status.loading') : t('keys.editor.noFastModels')" />
      </div>
    </ZScrollbar>
  </div>
</template>
