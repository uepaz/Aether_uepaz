import { beforeEach, describe, expect, it, vi } from 'vitest'

const { get, put } = vi.hoisted(() => ({ get: vi.fn(), put: vi.fn() }))
vi.mock('@/api/client', () => ({ default: { get, put } }))
import { modulesApi } from '@/api/modules'

describe('user module status API', () => {
  beforeEach(() => vi.resetAllMocks())

  it('saves both health visibility switches in one configuration write', async () => {
    const visibility = { user_enabled: false, admin_enabled: true }
    put.mockResolvedValue({ data: { value: visibility } })
    await expect(modulesApi.updateHealthMonitorVisibility(visibility)).resolves.toEqual(visibility)
    expect(put).toHaveBeenCalledExactlyOnceWith('/api/admin/system/configs/module.health_monitor.visibility', {
      value: visibility, description: '健康监控两端展示设置',
    })
  })

  it('loads minimal user status without calling the administrator endpoint', async () => {
    const statuses = { referral: { name: 'referral', available: true, enabled: true, active: true } }
    get.mockResolvedValue({ data: statuses })
    await expect(modulesApi.getUserStatus()).resolves.toEqual(statuses)
    expect(get).toHaveBeenCalledExactlyOnceWith('/api/modules/user-status')
  })
})
