import { afterEach, describe, expect, it, vi } from 'vitest'

vi.mock('@/config/demo', () => ({ isDemoMode: () => true, DEMO_ACCOUNTS: {} }))
import { handleMockRequest, setMockUserToken } from '../handler'
import { MOCK_MODULE_STATUSES } from '../data'

describe('remote control demo contract', () => {
  const original = structuredClone(MOCK_MODULE_STATUSES.vscodex)
  afterEach(() => {
    MOCK_MODULE_STATUSES.vscodex = structuredClone(original)
    setMockUserToken(null)
  })

  it('keeps user status and remote control access consistent with the administrator switch', async () => {
    for (const enabled of [true, false]) {
      setMockUserToken('demo-access-token-admin')
      await handleMockRequest({ method: 'PUT', url: '/api/admin/modules/status/vscodex/enabled', data: JSON.stringify({ enabled }) })
      setMockUserToken('demo-access-token-user')
      const status = (await handleMockRequest({ method: 'GET', url: '/api/modules/user-status' }))?.data as Record<string, Record<string, unknown>>
      expect(status.vscodex).toEqual({ name: 'vscodex', available: true, enabled, active: enabled })
      for (const [method, url] of [
        ['GET', '/api/users/me/vscodex/devices'],
        ['POST', '/api/users/me/vscodex/pairings'],
        ['POST', '/api/users/me/vscodex/ws-tickets'],
        ['POST', '/api/vscodex/pair'],
      ]) {
        const request = handleMockRequest({ method, url })
        if (enabled) expect((await request)?.status).toBe(200)
        else await expect(request).rejects.toMatchObject({ response: { status: 403 } })
      }
      setMockUserToken('demo-access-token-admin')
      expect((await handleMockRequest({ method: 'GET', url: '/api/admin/modules/status/vscodex' }))?.status).toBe(200)
    }
  })
})
