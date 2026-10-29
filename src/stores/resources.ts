import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  checkResources,
  getReleaseInfo,
  installResources,
  isTauri,
  type ReleaseInfo,
  type ResourceProgress,
  type ResourceStatus,
} from '@/services/backend'
import { useSettingsStore } from './settings'
import { useToastStore } from './toast'
import { errorMessage } from '@/utils/format'

export const useResourcesStore = defineStore('resources', () => {
  const settings = useSettingsStore()
  const toast = useToastStore()

  const status = ref<ResourceStatus | null>(null)
  const release = ref<ReleaseInfo | null>(null)
  const checking = ref(false)
  const downloading = ref(false)
  const error = ref('')
  const progress = ref<ResourceProgress>({ stage: 'done', downloaded: 0, total: 0 })
  const initialised = ref(false)

  let unlisten: UnlistenFn | null = null

  const installed = computed(() => Boolean(status.value?.installed && status.value?.valid))
  const canInstall = computed(() => Boolean(release.value) && !downloading.value)
  const progressPercent = computed(() => {
    if (progress.value.stage === 'done') return 100
    if (progress.value.total <= 0) return 0
    return Math.min(100, Math.round((progress.value.downloaded / progress.value.total) * 100))
  })

  async function refreshStatus() {
    try {
      status.value = await checkResources()
    } catch (caught) {
      error.value = errorMessage(caught)
    }
  }

  async function checkForRelease() {
    checking.value = true
    error.value = ''
    // The installed-status check is local and fast. Keep it independent from
    // the GitHub lookup so a slow network never makes installed resources look
    // missing.
    const statusTask = checkResources()
      .then((nextStatus) => {
        status.value = nextStatus
      })
      .catch((caught) => {
        error.value = errorMessage(caught)
      })
    const releaseTask = getReleaseInfo()
      .then((nextRelease) => {
        release.value = nextRelease
        if (!nextRelease) error.value = settings.t('settings.noRelease')
      })
      .catch((caught) => {
        error.value = errorMessage(caught)
        toast.error(settings.t('settings.resourceError'), errorMessage(caught))
      })
    try {
      await Promise.all([statusTask, releaseTask])
    } finally {
      checking.value = false
    }
  }

  async function init() {
    if (initialised.value) return
    initialised.value = true
    await checkForRelease()
  }

  async function install() {
    if (downloading.value) return
    downloading.value = true
    error.value = ''
    progress.value = { stage: 'fetching', downloaded: 0, total: 0 }

    if (isTauri) {
      unlisten = await listen<ResourceProgress>('resources://progress', (event) => {
        progress.value = event.payload
      })
    }

    try {
      const result = await installResources()
      status.value = result.status
      release.value = result.release
      progress.value = { stage: 'done', downloaded: 1, total: 1 }
      toast.success(settings.t('settings.downloadComplete'))
    } catch (caught) {
      error.value = errorMessage(caught)
      progress.value = { stage: 'done', downloaded: 0, total: 0 }
      toast.error(settings.t('settings.downloadFailed'), errorMessage(caught))
      throw caught
    } finally {
      downloading.value = false
      unlisten?.()
      unlisten = null
    }
  }

  return {
    status,
    release,
    checking,
    downloading,
    error,
    progress,
    initialised,
    installed,
    canInstall,
    progressPercent,
    init,
    refreshStatus,
    checkForRelease,
    install,
  }
})
