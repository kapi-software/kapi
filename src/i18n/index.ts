/**
 * @file i18n/index.ts
 * @description i18next 初始化：语言包为 TS 模块（规避本机 .json 加密，见 CLAUDE.md §0.1）
 * i18next initialization with TS locale modules
 * @author Kapi 开发团队 / Kapi Development Team
 * @created 2026-08-25
 * @updated 2026-08-25
 *
 * @changes
 * - 2026-08-25: Phase 1 随 i18n 需求创建
 */

import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import zhCN from './locales/zh-CN'
import enUS from './locales/en-US'

/** 支持的语言列表 / Supported languages */
export const SUPPORTED_LANGUAGES = ['zh-CN', 'en-US'] as const

/** 应用语言类型 / App language type */
export type AppLanguage = (typeof SUPPORTED_LANGUAGES)[number]

/**
 * 校验语言值是否受支持，不支持的值回退 zh-CN
 * Validate a language value, falling back to zh-CN when unsupported
 *
 * @param lang - 待校验语言值 / Language value to validate
 * @returns 受支持的语言 / A supported language
 *
 * @example
 * normalizeLanguage('en-US')  // => 'en-US'
 * normalizeLanguage('fr-FR')  // => 'zh-CN'
 */
export function normalizeLanguage(lang: string): AppLanguage {
  return (SUPPORTED_LANGUAGES as readonly string[]).includes(lang)
    ? (lang as AppLanguage)
    : 'zh-CN'
}

// React 已负责 XSS 转义，关闭 i18next 的 escapeValue
// React already handles XSS escaping, so disable i18next escapeValue
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
