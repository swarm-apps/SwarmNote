/**
 * 部署 base path 工具。
 *
 * Astro 的 `base` 配置只会自动改写 `_astro/*` 之类的内部资源；
 * 我们手写的 `<a href="/foo">` / `<img src="/logo.png">` 不会被处理。
 * 所以页面里所有指向站内资源的路径都应通过 `withBase()` 包一层。
 *
 * 切到自定义域名（base = '/'）后，本函数自动退化为恒等。
 */

/** 去尾斜杠的 base，例如 '/SwarmNote' 或 ''（自定义域名）。 */
export const BASE = (import.meta.env.BASE_URL ?? "/").replace(/\/+$/, "");

/**
 * 给站内绝对路径加上 base 前缀。
 * - http(s) / 锚点 / mailto / 已经包含 base：原样返回
 * - 相对路径（不以 / 开头）：原样返回（让浏览器按当前页解析）
 */
export function withBase(path: string): string {
  if (!path) return path;
  if (
    path.startsWith("http://") ||
    path.startsWith("https://") ||
    path.startsWith("//") ||
    path.startsWith("#") ||
    path.startsWith("mailto:") ||
    path.startsWith("tel:")
  ) {
    return path;
  }
  if (!path.startsWith("/")) return path;
  if (BASE && (path === BASE || path.startsWith(`${BASE}/`))) return path;
  return `${BASE}${path}`;
}
