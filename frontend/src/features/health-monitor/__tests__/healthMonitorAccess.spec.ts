import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, h, nextTick, type App } from 'vue'
import { createPinia, setActivePinia } from 'pinia'
import { createMemoryHistory, createRouter, type RouteLocationNormalized } from 'vue-router'
import { useModuleStore } from '@/stores/modules'
import { checkAdminAccess } from '@/router/guards/adminGuard'
import { checkModuleAccess } from '@/router/guards/moduleGuard'
import { useHealthMonitorAccess } from '../useHealthMonitorAccess'
import { healthMonitorModule } from './fixtures'

const { getAllStatus, getUserStatus } = vi.hoisted(() => ({ getAllStatus: vi.fn(), getUserStatus: vi.fn() }))
vi.mock('@/api/modules', () => ({ modulesApi: { getAllStatus, getUserStatus } }))
let app: App | undefined
beforeEach(() => { vi.resetAllMocks(); setActivePinia(createPinia()) })
afterEach(() => { app?.unmount(); app = undefined; vi.useRealTimers() })
function route(path: string, module?: string) {
  return { path, matched: [{ meta: { module } }] } as unknown as RouteLocationNormalized
}
function userStatus(enabled: boolean, user: boolean) {
  return { health_monitor: { name: 'health_monitor', available: true, enabled, active: enabled && user } }
}

describe('health monitor page access', () => {
  it.each([
    [false, true, true], [true, true, true], [true, true, false], [true, false, true], [true, false, false],
  ])('gates direct URLs with total=%s user=%s admin=%s', async (enabled, user, admin) => {
    getAllStatus.mockResolvedValue({ health_monitor: healthMonitorModule(enabled, user, admin) })
    getUserStatus.mockResolvedValue(userStatus(enabled, user))
    const store = useModuleStore()
    await expect(checkAdminAccess(route('/admin/health-monitor', 'health_monitor'), { canAccessAdmin: true } as never, store)).resolves.toBe(enabled && admin ? null : '/admin/dashboard')
    await expect(checkModuleAccess(route('/dashboard/endpoint-status', 'health_monitor'), store)).resolves.toBe(enabled && user ? null : '/dashboard')
    await expect(checkModuleAccess(route('/status', 'health_monitor'), store)).resolves.toBe(enabled && user ? null : '/')
    await expect(checkAdminAccess(route('/admin/modules'), { canAccessAdmin: true } as never, store)).resolves.toBeNull()
  })

  it('rechecks a previously enabled route and denies unavailable status', async () => {
    const store = useModuleStore()
    store.loaded = true
    store.modules = { health_monitor: healthMonitorModule() }
    getAllStatus.mockRejectedValue(new Error('unavailable'))
    await expect(checkAdminAccess(route('/admin/health-monitor', 'health_monitor'), { canAccessAdmin: true } as never, store)).resolves.toBe('/admin/dashboard')
    expect(store.adminHealthMonitorActive).toBe(false)
  })

  it.each([true, false])('unmounts an open page after a remote disable with admin=%s', async admin => {
    vi.useFakeTimers()
    getAllStatus.mockResolvedValue({ health_monitor: healthMonitorModule() })
    getUserStatus.mockResolvedValue(userStatus(true, true))
    const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/:pathMatch(.*)*', component: { render: () => null } }] })
    await router.push(admin ? '/admin/health-monitor' : '/dashboard/endpoint-status')
    const root = document.createElement('div')
    app = createApp({ setup() {
      const allowed = useHealthMonitorAccess(admin)
      return () => allowed.value ? h('p', '监控数据') : null
    } }).use(router)
    app.mount(root)
    await vi.advanceTimersByTimeAsync(0)
    await nextTick()
    expect(root.textContent).toBe('监控数据')
    getAllStatus.mockResolvedValue({ health_monitor: healthMonitorModule(true, true, false) })
    getUserStatus.mockResolvedValue(userStatus(true, false))
    await vi.advanceTimersByTimeAsync(30_000)
    await nextTick()
    expect(root.textContent).toBe('')
    expect(router.currentRoute.value.path).toBe(admin ? '/admin/dashboard' : '/dashboard')
  })
})
