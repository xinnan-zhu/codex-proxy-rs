<script setup lang="ts">
import { BaseCard, BaseSwitch } from '@codex-proxy/ui'

const enabled = defineModel<boolean>({ required: true })
</script>

<template>
  <BaseCard title="阻断降智对话" description="打开后，上游返回 312 字节 turn-state 时不把该响应发给客户端">
    <BaseSwitch v-model="enabled" class="justify-self-start" label="阻断降智对话" show-label />
  </BaseCard>
</template>
