import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, h, nextTick, ref, type App } from 'vue'
import { createPinia, setActivePinia } from 'pinia'
import { useModuleStore } from '@/stores/modules'
import { healthMonitorModule } from '@/features/health-monitor/__tests__/fixtures'
import HealthMonitorConfigDialog from '../HealthMonitorConfigDialog.vue'
import { setI18nLocale } from '@/i18n'

const { save } = vi.hoisted(() => ({ save: vi.fn() }))
vi.mock('@/api/modules', () => ({ modulesApi: { updateHealthMonitorVisibility: save } }))
vi.mock('@/components/ui', async () => {
  const { defineComponent, h } = await import('vue')
  const { default: Button } = await import('@/components/ui/button.vue')
  const { default: Switch } = await import('@/components/ui/switch.vue')
  return { Button, Switch, Dialog: defineComponent({
    props: { modelValue: Boolean },
    setup: (props, { slots }) => () => props.modelValue ? h('section', { role: 'dialog' }, [slots.default?.(), slots.footer?.()]) : null,
  }) }
})
let app: App | undefined
let root: HTMLElement
beforeEach(() => { vi.resetAllMocks(); setActivePinia(createPinia()); setI18nLocale('zh-CN') })
afterEach(() => { app?.unmount(); root?.remove() })
async function mount(enabled = true) {
  useModuleStore().modules = { health_monitor: healthMonitorModule(enabled) }
  const open = ref(false)
  root = document.createElement('div'); document.body.append(root)
  app = createApp({ setup: () => () => h(HealthMonitorConfigDialog, { modelValue: open.value, 'onUpdate:modelValue': (value: boolean) => { open.value = value } }) })
  app.mount(root); open.value = true; await nextTick()
}
function saveButton() { return [...root.querySelectorAll('button')].find(button => button.textContent?.trim() === '保存')! }
describe('health monitoring configuration', () => {
  it('keeps configuration editable with the total switch off and saves both switches together', async () => {
    await mount(false)
    expect(root.textContent).toContain('总开关已关闭')
    const switches = root.querySelectorAll<HTMLButtonElement>('[role="switch"]')
    expect(switches).toHaveLength(2)
    expect(switches[0].disabled).toBe(false)
    switches[0].click(); await nextTick()
    save.mockResolvedValue({ user_enabled: false, admin_enabled: true })
    saveButton().click()
    await vi.waitFor(() => expect(root.querySelector('[role="dialog"]')).toBeNull())
    expect(save).toHaveBeenCalledExactlyOnceWith({ user_enabled: false, admin_enabled: true })
    expect(useModuleStore().modules.health_monitor?.visibility).toEqual({ user_enabled: false, admin_enabled: true })
    expect(useModuleStore().isEnabled('health_monitor')).toBe(false)
  })
  it('retains persisted switches and keeps the dialog open after saving fails', async () => {
    await mount()
    root.querySelector<HTMLButtonElement>('[role="switch"]')!.click(); await nextTick()
    save.mockRejectedValue(new Error('failed'))
    saveButton().click()
    await vi.waitFor(() => expect(saveButton().disabled).toBe(false))
    expect(root.querySelector('[role="dialog"]')).not.toBeNull()
    expect(useModuleStore().modules.health_monitor?.visibility).toEqual({ user_enabled: true, admin_enabled: true })
  })
})
