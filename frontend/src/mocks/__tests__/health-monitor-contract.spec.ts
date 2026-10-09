import { afterEach, describe, expect, it, vi } from 'vitest'

vi.mock('@/config/demo', () => ({ isDemoMode: () => true, DEMO_ACCOUNTS: {} }))
import { handleMockRequest, setMockUserToken } from '../handler'
import { MOCK_MODULE_STATUSES, MOCK_SYSTEM_CONFIGS } from '../data'

describe('health monitor demo contract', () => {
  const originalModule = structuredClone(MOCK_MODULE_STATUSES.health_monitor)
  const originalConfigs = structuredClone(MOCK_SYSTEM_CONFIGS)
  afterEach(() => {
    MOCK_MODULE_STATUSES.health_monitor = structuredClone(originalModule)
    MOCK_SYSTEM_CONFIGS.splice(0, MOCK_SYSTEM_CONFIGS.length, ...structuredClone(originalConfigs))
    setMockUserToken(null)
  })

  it.each([
    [false, true, true], [true, true, true], [true, false, true],
    [true, true, false], [true, false, false],
  ])('gates both audiences with master=%s, user=%s, admin=%s', async (enabled, user, admin) => {
    setMockUserToken('demo-access-token-admin')
    await handleMockRequest({ method: 'PUT', url: '/api/admin/modules/status/health_monitor/enabled', data: JSON.stringify({ enabled }) })
    await handleMockRequest({ method: 'PUT', url: '/api/admin/system/configs/module.health_monitor.visibility', data: JSON.stringify({ value: { user_enabled: user, admin_enabled: admin } }) })
    for (const [url, allowed] of [
      ['/api/public/health/api-formats', enabled && user],
      ['/api/admin/endpoints/health/api-formats', enabled && admin],
      ['/api/users/me/health/v2/summary', enabled && user],
      ['/api/admin/endpoints/health/v2/summary', enabled && admin],
    ] as const) {
      const request = handleMockRequest({ method: 'GET', url })
      if (allowed) expect((await request)?.status).toBe(200)
      else await expect(request).rejects.toMatchObject({ response: { status: 403, data: { code: 'health_monitor_disabled' } } })
    }
    // 关闭管理端后仍可重新配置，不影响提供商健康摘要。
    expect((await handleMockRequest({ method: 'GET', url: '/api/admin/modules/status/health_monitor' }))?.status).toBe(200)
    expect((await handleMockRequest({ method: 'GET', url: '/api/admin/endpoints/health/summary' }))?.status).toBe(200)
    expect((await handleMockRequest({ method: 'GET', url: '/api/admin/endpoints/health/v2/publication' }))?.status).toBe(200)
    setMockUserToken('demo-access-token-user')
    const status = (await handleMockRequest({ method: 'GET', url: '/api/modules/user-status' }))?.data as Record<string, Record<string, unknown>>
    expect(status.health_monitor.active).toBe(enabled && user)
    expect(Object.keys(status.health_monitor).sort()).toEqual(['active', 'available', 'enabled', 'name'])
  })

  it('returns module snapshots so saving configuration cannot silently mutate earlier responses', async () => {
    setMockUserToken('demo-access-token-admin')
    const previous = (await handleMockRequest({ method: 'GET', url: '/api/admin/modules/status' }))?.data as typeof MOCK_MODULE_STATUSES
    await handleMockRequest({ method: 'PUT', url: '/api/admin/system/configs/module.health_monitor.visibility', data: JSON.stringify({ value: { user_enabled: true, admin_enabled: false } }) })
    expect(previous.health_monitor.visibility).toEqual(originalModule.visibility)
    const current = (await handleMockRequest({ method: 'GET', url: '/api/admin/modules/status' }))?.data as typeof MOCK_MODULE_STATUSES
    expect(current.health_monitor.visibility?.admin_enabled).toBe(false)
  })

  it('rejects incomplete visibility without changing saved settings', async () => {
    setMockUserToken('demo-access-token-admin')
    await expect(handleMockRequest({ method: 'PUT', url: '/api/admin/system/configs/module.health_monitor.visibility', data: JSON.stringify({ value: { user_enabled: false } }) }))
      .rejects.toMatchObject({ response: { status: 400 } })
    expect(MOCK_MODULE_STATUSES.health_monitor.visibility).toEqual(originalModule.visibility)
    expect(MOCK_SYSTEM_CONFIGS).toEqual(originalConfigs)
  })
})
