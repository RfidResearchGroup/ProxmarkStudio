import { defineStore } from 'pinia'
import { useSettingsStore } from '@/stores/settings'
import type { TranslationKey } from '@/i18n/en'

export const useI18n = () => {
  const settings = useSettingsStore()
  const t = (key: TranslationKey, params?: Record<string, string | number>) => settings.t(key, params)
  const locale = settings.resolvedLocale
  const isChinese = locale === 'zh'
  return { t, locale, isChinese, settings }
}

export type { TranslationKey }
