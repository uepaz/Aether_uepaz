import { describe, expect, it } from 'vitest'
import { formatClientFamily } from '../clientFamily'

describe('formatClientFamily', () => {
  it('labels supported clients and SDKs', () => {
    expect(formatClientFamily('qwen_code')).toBe('Qwen Code')
    expect(formatClientFamily('roo_code')).toBe('Roo Code')
    expect(formatClientFamily('kilocode')).toBe('KiloCode')
    expect(formatClientFamily('cherrystudio')).toBe('Cherry Studio')
    expect(formatClientFamily('openui')).toBe('OpenUI')
    expect(formatClientFamily('workbuddy')).toBe('WorkBuddy')
    expect(formatClientFamily('deepseek_harness')).toBe('DeepSeek Harness')
    expect(formatClientFamily('hermes_agent')).toBe('Hermes Agent')
    expect(formatClientFamily('openai_python_sdk')).toBe('OpenAI Python SDK')
    expect(formatClientFamily('anthropic_js_sdk')).toBe('Anthropic JS SDK')
  })

  it('displays a valid client declaration without a name mapping', () => {
    expect(formatClientFamily(' My-Agent_2 ')).toBe('my-agent_2')
  })

  it('renders empty or malformed family values as unknown', () => {
    expect(formatClientFamily(null)).toBe('unknown')
    expect(formatClientFamily('')).toBe('unknown')
    expect(formatClientFamily('my agent')).toBe('unknown')
    expect(formatClientFamily('my/agent')).toBe('unknown')
    expect(formatClientFamily('-my-agent')).toBe('unknown')
    expect(formatClientFamily('x'.repeat(65))).toBe('unknown')
  })
})
