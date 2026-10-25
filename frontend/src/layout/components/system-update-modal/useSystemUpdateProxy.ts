import { computed, shallowRef } from 'vue'
import { getSystemUpdateProxy, updateSystemUpdateProxy } from '@/api'
import { useProxyCatalog } from '@/composables/useProxyCatalog'
import { t } from '@/i18n'
import { notification as ZNotification } from '@/utils/notification'
import { errorMessage } from '@/utils/operation'

const DIRECT = 'direct'

export function useSystemUpdateProxy() {
  const { proxies, loading: proxiesLoading, loadProxies } = useProxyCatalog()
  const selectedProxyId = shallowRef<string | null>(null)
  const loadingSelection = shallowRef(false)
  const saving = shallowRef(false)

  const options = computed(() => {
    const items = [
      { label: t('systemUpdate.downloadProxy.direct'), value: DIRECT, description: t('systemUpdate.downloadProxy.noProxy') },
      ...proxies.value.map(proxy => ({
        label: `${proxy.name}${!proxy.enabled ? t('systemUpdate.downloadProxy.disabled') : proxy.lastTest?.success ? '' : proxy.lastTest ? t('systemUpdate.downloadProxy.failed') : t('systemUpdate.downloadProxy.untested')}`,
        value: proxy.id,
        description: !proxy.enabled && proxy.id === selectedProxyId.value ? t('systemUpdate.downloadProxy.currentlyDirect') : proxy.endpoint,
        disabled: !proxy.enabled,
      })),
    ]
    const selected = selectedProxyId.value
    if (selected && !proxies.value.some(proxy => proxy.id === selected))
      items.push({ label: t('systemUpdate.downloadProxy.selected'), value: selected, description: selected })
    return items
  })

  const selection = computed({
    get: () => selectedProxyId.value ?? DIRECT,
    set: (value: string) => {
      void save(value === DIRECT ? null : value)
    },
  })

  const busy = computed(() => proxiesLoading.value || loadingSelection.value || saving.value)

  async function load() {
    loadingSelection.value = true
    try {
      const [data] = await Promise.all([getSystemUpdateProxy(), loadProxies()])
      selectedProxyId.value = data.proxyId
    }
    catch {}
    finally {
      loadingSelection.value = false
    }
  }

  async function save(proxyId: string | null) {
    if (saving.value || proxyId === selectedProxyId.value)
      return
    saving.value = true
    try {
      const data = await updateSystemUpdateProxy({ proxyId })
      selectedProxyId.value = data.proxyId
      ZNotification.success({ message: data.proxyId ? t('systemUpdate.downloadProxy.savedProxy') : t('systemUpdate.downloadProxy.savedDirect') })
    }
    catch (error: unknown) {
      ZNotification.error({ message: errorMessage(error, t('systemUpdate.downloadProxy.saveFailed')) })
    }
    finally {
      saving.value = false
    }
  }

  return { options, selection, busy, load }
}
