import { en } from './locales/en'
import { zhCN } from './locales/zhCN'
import type { AppLanguage, SupportedLanguage, TranslationSet } from './types'

export type { AppLanguage, I18nShape, SupportedLanguage, TranslationSet } from './types'

export const SUPPORTED_LANGUAGES: SupportedLanguage[] = [
  { code: 'zh-CN', label: 'Chinese', nativeLabel: '简体中文' },
  { code: 'en', label: 'English', nativeLabel: 'English' },
]

const translations: Record<AppLanguage, TranslationSet> = {
  'zh-CN': zhCN,
  en,
}

export const LANGUAGE_STORAGE_KEY = 'codex-counter.language'

export function getTranslations(language: AppLanguage) {
  return translations[language]
}

export function getNumberLocale(language: AppLanguage) {
  return language === 'zh-CN' ? 'zh-CN' : 'en-US'
}

export function getDateTimeLocale(language: AppLanguage) {
  return language === 'zh-CN' ? 'zh-CN' : 'en-SG'
}

export function getRelativeTimeLocale(language: AppLanguage) {
  return language === 'zh-CN' ? 'zh-CN' : 'en'
}

export function readStoredLanguage(): AppLanguage {
  if (typeof window === 'undefined') return 'zh-CN'
  const stored = window.localStorage.getItem(LANGUAGE_STORAGE_KEY)
  return stored === 'en' || stored === 'zh-CN' ? stored : 'zh-CN'
}
