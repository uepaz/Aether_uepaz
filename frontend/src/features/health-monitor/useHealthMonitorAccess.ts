import { computed, onMounted, onUnmounted, ref, toValue, watch, type MaybeRefOrGetter } from 'vue'
import { useRouter } from 'vue-router'
import { useModuleStore } from '@/stores/modules'
import { useToast } from '@/composables/useToast'
import { useI18n } from '@/i18n'

/** 页面保持打开时也重新确认展示许可，关闭后卸载内容并停止刷新。 */
export function useHealthMonitorAccess(admin: MaybeRefOrGetter<boolean>, publicPage = false) {
  const store = useModuleStore()
  const router = useRouter()
  const { info } = useToast()
  const { legacyT } = useI18n()
  const checked = ref(false)
  const allowed = computed(() => toValue(admin) ? store.adminHealthMonitorActive : store.isUserActive('health_monitor'))
  let timer: ReturnType<typeof setInterval> | undefined
  let disposed = false
  async function refresh() {
    try {
      await (toValue(admin) ? store.fetchModules() : store.fetchUserModules())
    } catch {
      // Store 清除健康监控许可，状态不可读时页面默认拒绝展示。
    } finally {
      if (!disposed) checked.value = true
    }
  }
  watch([checked, allowed], ([ready, active]) => {
    if (ready && !active) {
      info(legacyT('健康监控已关闭'))
      void router.replace(publicPage ? '/' : toValue(admin) ? '/admin/dashboard' : '/dashboard')
    }
  })
  onMounted(() => {
    void refresh()
    timer = setInterval(() => { void refresh() }, 30_000)
    window.addEventListener('focus', refresh)
  })
  onUnmounted(() => {
    disposed = true
    clearInterval(timer)
    window.removeEventListener('focus', refresh)
  })
  return computed(() => checked.value && allowed.value)
}
