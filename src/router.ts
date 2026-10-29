import { createRouter, createWebHashHistory } from 'vue-router'
import HomeView from '@/views/HomeView.vue'
import { useSettingsStore } from '@/stores/settings'
import type { TranslationKey } from '@/i18n/en'

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: '/',
      name: 'home',
      component: HomeView,
      meta: { title: 'nav.home' },
    },
    {
      path: '/files',
      name: 'files',
      component: () => import('@/views/FilesView.vue'),
      meta: { title: 'nav.files' },
    },
    {
      path: '/settings',
      name: 'settings',
      component: () => import('@/views/SettingsView.vue'),
      meta: { title: 'nav.settings' },
    },
    {
      path: '/terminal',
      name: 'terminal',
      component: () => import('@/views/TerminalView.vue'),
      meta: { title: 'nav.terminal' },
    },
    {
      path: '/:pathMatch(.*)*',
      redirect: '/',
    },
  ],
  scrollBehavior() {
    return { top: 0 }
  },
})

router.afterEach((to) => {
  const settings = useSettingsStore()
  const title = to.meta?.title as TranslationKey | undefined
  document.title = title ? `${settings.t(title)} · Proxmark Studio` : 'Proxmark Studio'
})

const viewPreloaders = [
  () => import('@/views/FilesView.vue'),
  () => import('@/views/SettingsView.vue'),
  () => import('@/views/TerminalView.vue'),
]

let viewsPreloaded = false

/** Warm the route chunks after the first screen so page switches feel instant. */
export function preloadViews() {
  if (viewsPreloaded) return
  viewsPreloaded = true
  // xterm and the terminal view are heavy; on a phone preloading them right
  // after launch causes more jank than it saves.
  const mobile = window.matchMedia('(max-width: 860px)').matches
  const preloaders = mobile ? viewPreloaders.slice(0, 2) : viewPreloaders
  window.setTimeout(() => {
    for (const load of preloaders) void load()
  }, 700)
}
