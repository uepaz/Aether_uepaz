import { AxiosError } from 'axios'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createApp, h, ref, type App } from 'vue'

import type { RequestDetail } from '@/api/dashboard'
import type { RequestTrace } from '@/api/requestTrace'
import RequestDetailDrawer from '../RequestDetailDrawer.vue'

const get = vi.hoisted(() => vi.fn())
vi.mock('@/api/client', () => ({ default: { get } }))

let app: App | undefined
let root: HTMLDivElement | undefined

afterEach(() => {
  app?.unmount()
  root?.remove()
  get.mockReset()
})

describe('RequestDetailDrawer trace lookup', () => {
  it.each([
    { name: 'Responses', apiFormat: 'openai:responses', requestId: 'gateway-id', lookupId: 'gateway-id' },
    { name: 'Chat converted to Responses', apiFormat: 'openai:chat', requestId: 'gateway-id', lookupId: 'gateway-id' },
    { name: 'legacy record without a request ID', apiFormat: 'openai:chat', requestId: undefined, lookupId: 'client-trace' },
    { name: 'legacy record with a blank request ID', apiFormat: 'openai:chat', requestId: ' ', lookupId: 'client-trace' },
  ])('loads the candidate trace for $name', async ({ name, apiFormat, requestId, lookupId }) => {
    const detail = {
      id: `usage-${name.toLowerCase().replaceAll(' ', '-')}`,
      request_id: requestId,
      user: { id: 'user-1', username: 'test-user', email: 'test@example.com' },
      api_key: { id: 'key-1', name: 'test-key', display: 'test-key' },
      provider: 'usage-provider',
      model: 'test-model',
      api_format: apiFormat,
      endpoint_api_format: 'openai:responses',
      has_format_conversion: apiFormat === 'openai:chat',
      tokens: { input: 10, output: 20, total: 30 },
      cost: { input: 0, output: 0, total: 0 },
      request_type: 'chat',
      is_stream: true,
      status: 'completed',
      status_code: 200,
      response_time_ms: 100,
      created_at: '2026-11-12T07:49:14Z',
      trace: { trace_id: 'client-trace' },
      metadata: { trace_id: 'client-trace' },
      request_headers: { 'x-trace-id': 'client-trace' },
    } satisfies Omit<RequestDetail, 'request_id'> & { request_id: string | undefined }

    const trace: RequestTrace = {
      request_id: lookupId,
      total_candidates: 1,
      final_status: 'success',
      total_latency_ms: 100,
      candidates: [{
        id: 'candidate-1',
        request_id: lookupId,
        candidate_index: 0,
        retry_index: 0,
        provider_name: 'trace-candidate-provider',
        status: 'success',
        status_code: 200,
        is_cached: false,
        created_at: detail.created_at,
      }],
    }
    const tracePath = `/api/admin/monitoring/trace/${lookupId}`
    get.mockImplementation(async (url: string) => {
      if (url === `/api/admin/usage/${detail.id}`) return { data: detail }
      if (url === tracePath) return { data: trace }
      throw Object.assign(new AxiosError('Trace not found'), { response: { status: 404 } })
    })

    const isOpen = ref(false)
    root = document.createElement('div')
    document.body.appendChild(root)
    app = createApp({
      setup: () => () => h(RequestDetailDrawer, { isOpen: isOpen.value, requestId: detail.id }),
    })
    app.mount(root)
    isOpen.value = true

    await vi.waitFor(() => expect(get).toHaveBeenCalledTimes(2))
    expect(get).toHaveBeenLastCalledWith(tracePath, { params: { attempted_only: false } })
    await vi.waitFor(() => expect(document.body.textContent).toContain('trace-candidate-provider'))
  })
})
