<template>
  <Dialog
    v-model="open"
    :title="t('healthMonitor.settings.title')"
    :persistent="saving"
    size="md"
  >
    <div class="space-y-4 py-2">
      <p class="text-sm text-muted-foreground">
        {{ t('healthMonitor.settings.description') }}
      </p>
      <p
        v-if="!store.isEnabled('health_monitor')"
        class="rounded-lg bg-muted px-3 py-2 text-sm text-muted-foreground"
      >
        {{ t('healthMonitor.settings.disabledHint') }}
      </p>
      <div
        v-for="item in controls"
        :key="item.key"
        class="flex items-center justify-between gap-4 rounded-xl border p-4"
      >
        <div>
          <label
            :for="item.key"
            class="font-medium"
          >{{ t(item.title) }}</label>
          <p class="mt-1 text-xs text-muted-foreground">
            {{ t(item.description) }}
          </p>
        </div>
        <Switch
          :id="item.key"
          v-model="draft[item.key]"
          :aria-label="t(item.title)"
          :disabled="saving"
        />
      </div>
    </div>
    <template #footer>
      <Button
        variant="outline"
        :disabled="saving"
        @click="open = false"
      >
        {{ t('common.cancel') }}
      </Button>
      <Button
        :disabled="saving"
        @click="save"
      >
        {{ saving ? t('ui.saving') : t('ui.save') }}
      </Button>
    </template>
  </Dialog>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { Button, Dialog, Switch } from '@/components/ui'
import { modulesApi, type HealthMonitorVisibility } from '@/api/modules'
import { useModuleStore } from '@/stores/modules'
import { useToast } from '@/composables/useToast'
import { getErrorMessage } from '@/types/api-error'
import { useI18n } from '@/i18n'

const open = defineModel<boolean>({ required: true })
const store = useModuleStore()
const { t } = useI18n()
const { success, error } = useToast()
const saving = ref(false)
const draft = ref<HealthMonitorVisibility>({ user_enabled: true, admin_enabled: true })
const controls = [
  { key: 'user_enabled', title: 'healthMonitor.settings.userTitle', description: 'healthMonitor.settings.userDescription' },
  { key: 'admin_enabled', title: 'healthMonitor.settings.adminTitle', description: 'healthMonitor.settings.adminDescription' },
] as const

watch(open, value => {
  if (value) draft.value = { ...(store.modules.health_monitor?.visibility ?? { user_enabled: true, admin_enabled: true }) }
})

async function save() {
  if (saving.value) return
  saving.value = true
  try {
    const visibility = await modulesApi.updateHealthMonitorVisibility({ ...draft.value })
    // 保存成功即同步当前会话，后续状态刷新失败也不回退到旧设置。
    if (store.modules.health_monitor) store.modules.health_monitor.visibility = visibility
    success(t('healthMonitor.settings.saved'))
    open.value = false
  } catch (cause) {
    error(getErrorMessage(cause, t('healthMonitor.settings.saveFailed')))
  } finally {
    saving.value = false
  }
}
</script>
