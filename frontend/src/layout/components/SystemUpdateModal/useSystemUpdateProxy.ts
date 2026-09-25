import { toast } from '@codex-proxy/ui'
import { computed, shallowRef } from 'vue'
import { getSystemUpdateProxy, updateSystemUpdateProxy } from '@/api'
import { useProxyCatalog } from '@/composables/useProxyCatalog'
import { errorMessage } from '@/utils/async'

const DIRECT = 'direct'

export function useSystemUpdateProxy() {
  const { proxies, loading: proxiesLoading, loadProxies } = useProxyCatalog()
  const selectedProxyId = shallowRef<string | null>(null)
  const loadingSelection = shallowRef(false)
  const saving = shallowRef(false)

  const options = computed(() => {
    const items = [
      { label: '直连', value: DIRECT, description: '不使用代理' },
      ...proxies.value.map(proxy => ({
        label: `${proxy.name}${proxy.lastTest?.success ? '' : proxy.lastTest ? '（测试失败）' : '（未测试）'}`,
        value: proxy.id,
        description: proxy.endpoint,
      })),
    ]
    const selected = selectedProxyId.value
    if (selected && !proxies.value.some(proxy => proxy.id === selected))
      items.push({ label: '已选代理', value: selected, description: selected })
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
      toast.success(data.proxyId ? '更新将通过所选代理下载' : '更新将直连下载')
    }
    catch (error: unknown) {
      toast.error(errorMessage(error, '保存下载代理失败'))
    }
    finally {
      saving.value = false
    }
  }

  return { options, selection, busy, load }
}
