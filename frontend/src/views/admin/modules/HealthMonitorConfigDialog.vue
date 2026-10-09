<template>
  <Dialog
    v-model="open"
    :title="legacyT('健康监控配置')"
    :persistent="saving"
    size="md"
  >
    <div class="space-y-4 py-2">
      <p class="text-sm text-muted-foreground">
        {{ legacyT('控制两端健康监控导航和页面展示，不影响健康统计采集、熔断和自动恢复。') }}
      </p>
      <p
        v-if="!store.isEnabled('health_monitor')"
        class="rounded-lg bg-muted px-3 py-2 text-sm text-muted-foreground"
      >
        {{ legacyT('健康监控总开关已关闭，两端设置将在启用模块后生效。') }}
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
          >{{ legacyT(item.title) }}</label>
          <p class="mt-1 text-xs text-muted-foreground">
            {{ legacyT(item.description) }}
          </p>
        </div>
        <Switch
          :id="item.key"
          v-model="draft[item.key]"
          :aria-label="legacyT(item.title)"
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
        {{ legacyT('取消') }}
      </Button>
      <Button
        :disabled="saving"
        @click="save"
      >
        {{ legacyT(saving ? '保存中...' : '保存') }}
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
const { legacyT } = useI18n()
const { success, error } = useToast()
const saving = ref(false)
const draft = ref<HealthMonitorVisibility>({ user_enabled: true, admin_enabled: true })
const controls = [
  { key: 'user_enabled', title: '用户端健康监控', description: '控制用户端健康监控及公开服务状态页。' },
  { key: 'admin_enabled', title: '管理端健康监控', description: '控制管理端健康监控及相关跳转入口。' },
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
    success(legacyT('健康监控配置已保存'))
    open.value = false
  } catch (cause) {
    error(getErrorMessage(cause, legacyT('保存健康监控配置失败')))
  } finally {
    saving.value = false
  }
}
</script>
