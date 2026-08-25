// i18next 初始化：语言包为 TS 模块（src/i18n/locales/）
// i18next initialization with TS locale modules (src/i18n/locales/)
import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import zhCN from './locales/zh-CN'
import enUS from './locales/en-US'

// 支持的语言列表
// Supported languages
export const SUPPORTED_LANGUAGES = ['zh-CN', 'en-US'] as const

// 应用语言类型
// App language type
export type AppLanguage = (typeof SUPPORTED_LANGUAGES)[number]

// 校验语言值是否受支持，不支持的值回退 zh-CN
// Validate a language value, falling back to zh-CN when unsupported
// normalizeLanguage('en-US') => 'en-US'; normalizeLanguage('fr-FR') => 'zh-CN'
export function normalizeLanguage(lang: string): AppLanguage {
  return (SUPPORTED_LANGUAGES as readonly string[]).includes(lang)
    ? (lang as AppLanguage)
    : 'zh-CN'
}

// React 已负责 XSS 转义，关闭 i18next 的 escapeValue
// React already escapes, so disable i18next escapeValue
i18n.use(initReactI18next).init({
  resources: {
    'zh-CN': { translation: zhCN },
    'en-US': { translation: enUS },
  },
  lng: 'zh-CN',
  fallbackLng: 'zh-CN',
  interpolation: {
    escapeValue: false,
  },
})

export default i18n
