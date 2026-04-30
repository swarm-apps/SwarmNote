/**
 * i18n 工具函数。
 *
 * 路由策略（astro.config.mjs i18n）：
 *   /          → 中文（默认，无前缀）
 *   /en/       → 英文
 *
 * 字典存放：src/i18n/zh.json、src/i18n/en.json
 */

import zh from "./zh.json";
import en from "./en.json";

export const locales = ["zh", "en"] as const;
export type Locale = (typeof locales)[number];

export const defaultLocale: Locale = "zh";

const dictionaries = { zh, en } as const;
export type Dictionary = typeof zh;

/** 从 URL 推导当前 locale。无前缀视为默认 zh。 */
export function getLangFromUrl(url: URL): Locale {
  const [, segment] = url.pathname.split("/");
  if (segment && locales.includes(segment as Locale)) {
    return segment as Locale;
  }
  return defaultLocale;
}

/** 嵌套键访问：t("hero.h1") → dict.hero.h1 */
type Path = string;
function get(obj: unknown, path: Path): string {
  return path.split(".").reduce<unknown>((acc, k) => {
    if (acc && typeof acc === "object" && k in (acc as Record<string, unknown>)) {
      return (acc as Record<string, unknown>)[k];
    }
    return undefined;
  }, obj) as string;
}

/** 返回 t(key) 函数。fallback 顺序：当前 locale → 默认 locale → 原 key。 */
export function useTranslations(lang: Locale) {
  const dict = dictionaries[lang];
  const fallback = dictionaries[defaultLocale];
  return function t(key: Path): string {
    const value = get(dict, key) ?? get(fallback, key);
    return value ?? key;
  };
}

/** 给当前 path 生成 hreflang 链接对（含 x-default）。 */
export function getLocaleAlternates(currentPath: string, siteOrigin: string): Array<{
  hreflang: string;
  href: string;
}> {
  // 去掉前导 locale 段，保留语言无关的 path
  const stripped = stripLocalePrefix(currentPath);
  const zhUrl = new URL(stripped, siteOrigin).toString();
  const enUrl = new URL(`/en${stripped === "/" ? "/" : stripped}`, siteOrigin).toString();
  return [
    { hreflang: "zh", href: zhUrl },
    { hreflang: "en", href: enUrl },
    { hreflang: "x-default", href: zhUrl },
  ];
}

/** /en/download → /download；/ → / */
export function stripLocalePrefix(path: string): string {
  for (const loc of locales) {
    if (loc === defaultLocale) continue;
    if (path === `/${loc}` || path === `/${loc}/`) return "/";
    if (path.startsWith(`/${loc}/`)) return path.slice(`/${loc}`.length);
  }
  return path;
}

/** 切换 locale 时保持当前 path：/download + en → /en/download；/en/download + zh → /download */
export function switchLocale(currentPath: string, target: Locale): string {
  const stripped = stripLocalePrefix(currentPath);
  if (target === defaultLocale) return stripped;
  return `/${target}${stripped === "/" ? "/" : stripped}`;
}
