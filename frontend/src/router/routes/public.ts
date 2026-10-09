import type { RouteRecordRaw } from 'vue-router'
import { view } from './helpers'

export const publicRoutes: RouteRecordRaw[] = [
  {
    path: '/status',
    name: 'PublicStatus',
    component: view(() => import('@/views/public/Status.vue')),
    meta: { requiresAuth: false }
  },
  {
    path: '/',
    name: 'Home',
    component: view(() => import('@/views/public/Home.vue')),
    meta: { requiresAuth: false }
  },
  {
    path: '/register',
    name: 'RegisterEntry',
    component: view(() => import('@/views/public/Home.vue')),
    meta: { requiresAuth: false }
  },
  {
    path: '/privacy-policy',
    name: 'PrivacyPolicy',
    component: view(() => import('@/views/public/PrivacyPolicy.vue')),
    meta: { requiresAuth: false }
  },
  {
    path: '/logo-demo',
    name: 'LogoColorDemo',
    component: view(() => import('@/views/public/LogoColorDemo.vue')),
    meta: { requiresAuth: false }
  },
  {
    path: '/auth/callback',
    name: 'AuthCallback',
    component: view(() => import('@/views/public/AuthCallback.vue')),
    meta: { requiresAuth: false }
  }
]
