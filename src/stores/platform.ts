import { defineStore } from 'pinia'
import { getPlatformInfo, getPaths, type PlatformInfo, type PathInfo } from '@/services/backend'

export const usePlatformStore = defineStore('platform', {
  state: () => ({
    info: {
      os: 'web',
      arch: 'unknown',
      family: 'desktop',
      version: '0.0.0-local',
      tauriVersion: '2',
      capabilities: {
        usbSerial: false,
        ble: false,
        spp: false,
        share: false,
        nativeFiles: false,
        desktop: true,
      },
    } as PlatformInfo,
    paths: {
      workDir: '',
      pm3Dir: '',
      resourcesDir: '',
    } as PathInfo,
    ready: false,
  }),
  getters: {
    isMobile: (state) => state.info.family === 'mobile',
    isDesktop: (state) => state.info.family !== 'mobile',
    osLabel: (state) => {
      const labels: Record<string, string> = {
        windows: 'Windows',
        macos: 'macOS',
        linux: 'Linux',
        android: 'Android',
        ios: 'iOS',
        web: 'Web preview',
      }
      return labels[state.info.os] ?? state.info.os
    },
  },
  actions: {
    async init() {
      if (this.ready) return
      try {
        const [info, paths] = await Promise.all([getPlatformInfo(), getPaths()])
        this.info = info
        this.paths = paths
      } finally {
        this.ready = true
      }
    },
    async refreshPaths() {
      this.paths = await getPaths()
    },
  },
})
