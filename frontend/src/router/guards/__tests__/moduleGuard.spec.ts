import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { createMemoryHistory, createRouter, type RouteLocationNormalized } from 'vue-router'
import { dashboardRoutes } from '@/router/routes/dashboard'
import { useModuleStore } from '@/stores/modules'
import { checkModuleAccess } from '@/router/guards/moduleGuard'

const { getUserStatus, getAllStatus } = vi.hoisted(() => ({ getUserStatus: vi.fn(), getAllStatus: vi.fn() }))
vi.mock('@/api/modules', () => ({ modulesApi: { getUserStatus, getAllStatus } }))

describe('user module route guard', () => {
  const route = { matched: [{ meta: { module: 'management_tokens' } }] } as unknown as RouteLocationNormalized
  beforeEach(() => {
    vi.resetAllMocks()
    setActivePinia(createPinia())
  })

  it.each([true, false])('checks user status for active=%s', async active => {
    getUserStatus.mockResolvedValue({ management_tokens: { name: 'management_tokens', available: true, enabled: active, active } })
    await expect(checkModuleAccess(route, useModuleStore())).resolves.toBe(active ? null : '/dashboard')
    expect(getUserStatus).toHaveBeenCalledTimes(1)
    expect(getAllStatus).not.toHaveBeenCalled()
  })

  it('denies access when public status cannot be read', async () => {
    getUserStatus.mockRejectedValue(new Error('unavailable'))
    await expect(checkModuleAccess(route, useModuleStore())).resolves.toBe('/dashboard')
  })

  it.each([true, false])('guards the actual remote control URL with active=%s', async active => {
    getUserStatus.mockResolvedValue({ vscodex: { name: 'vscodex', available: true, enabled: active, active } })
    const router = createRouter({ history: createMemoryHistory(), routes: dashboardRoutes })
    const destination = router.resolve('/dashboard/vscodex') as unknown as RouteLocationNormalized
    await expect(checkModuleAccess(destination, useModuleStore())).resolves.toBe(active ? null : '/dashboard')
  })

  it('refreshes remote control permission even when an earlier enabled status is cached', async () => {
    const store = useModuleStore()
    store.userLoaded = true
    store.userModules = { vscodex: { name: 'vscodex', available: true, enabled: true, active: true } }
    getUserStatus.mockResolvedValue({ vscodex: { name: 'vscodex', available: true, enabled: false, active: false } })
    const router = createRouter({ history: createMemoryHistory(), routes: dashboardRoutes })
    await expect(checkModuleAccess(router.resolve('/dashboard/vscodex') as unknown as RouteLocationNormalized, store)).resolves.toBe('/dashboard')
    expect(getUserStatus).toHaveBeenCalledTimes(1)
  })
})
