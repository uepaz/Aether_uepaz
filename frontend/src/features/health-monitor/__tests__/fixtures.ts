import type { ModuleStatus } from '@/api/modules'

export function healthMonitorModule(enabled = true, user = true, admin = true): ModuleStatus {
  return {
    name: 'health_monitor', available: true, enabled, active: enabled,
    config_validated: true, config_error: null, display_name: '健康监控',
    description: '', category: 'monitoring', health: 'healthy',
    admin_route: null, admin_menu_group: null, admin_menu_icon: 'Activity', admin_menu_order: 0,
    visibility: { user_enabled: user, admin_enabled: admin },
  }
}
