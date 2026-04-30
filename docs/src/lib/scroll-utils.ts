/**
 * 滚动相关辅助：环境探测、ScrollTrigger 通用注册、清理。
 * 浏览器端模块：在 React island / 客户端脚本里使用。
 */

import { gsap } from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";

/** prefers-reduced-motion: reduce 用户偏好。SSR 安全（默认 false）。 */
export function prefersReducedMotion(): boolean {
  if (typeof window === "undefined") return false;
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/**
 * 触摸设备 / 移动端：(pointer: coarse) 或 屏宽 < 768。
 * 用于跳过 Lenis（避免与 iOS 原生回弹冲突）+ 降级 Hero 蜂群实例数。
 */
export function isCoarseOrNarrow(): boolean {
  if (typeof window === "undefined") return false;
  const coarse = window.matchMedia("(pointer: coarse)").matches;
  const narrow = window.innerWidth < 768;
  return coarse || narrow;
}

/** 估算设备能力档：用于 Hero 蜂群实例数与阴影开关。 */
export function deviceTier(): "low" | "high" {
  if (typeof window === "undefined") return "high";
  const memory = (navigator as Navigator & { deviceMemory?: number }).deviceMemory;
  if (typeof memory === "number" && memory < 4) return "low";
  if (window.innerWidth < 768) return "low";
  return "high";
}

/** WebGL 可用性（Hero 蜂群挂载前必检）。 */
export function isWebGLAvailable(): boolean {
  if (typeof window === "undefined") return false;
  try {
    const canvas = document.createElement("canvas");
    return !!(
      window.WebGLRenderingContext &&
      (canvas.getContext("webgl2") || canvas.getContext("webgl"))
    );
  } catch {
    return false;
  }
}

let pluginsRegistered = false;

/** 幂等注册 GSAP ScrollTrigger 插件。多次调用安全。 */
export function ensureScrollTrigger(): typeof ScrollTrigger {
  if (!pluginsRegistered) {
    gsap.registerPlugin(ScrollTrigger);
    pluginsRegistered = true;
  }
  return ScrollTrigger;
}

/**
 * 注册一段滚动驱动的 timeline。trigger 离开视口后默认 kill。
 * reduced-motion 时直接 no-op（动画归零）。
 */
export interface RegisterOptions {
  trigger: Element | string;
  start?: string;
  end?: string;
  scrub?: boolean | number;
  pin?: boolean;
  onEnter?: () => void;
  onLeave?: () => void;
  build: (tl: gsap.core.Timeline) => void;
}

export function registerScrollSection(opts: RegisterOptions): ScrollTrigger | null {
  if (prefersReducedMotion()) return null;
  ensureScrollTrigger();
  const tl = gsap.timeline({
    scrollTrigger: {
      trigger: opts.trigger,
      start: opts.start ?? "top 80%",
      end: opts.end ?? "bottom 20%",
      scrub: opts.scrub ?? false,
      pin: opts.pin ?? false,
      onEnter: opts.onEnter,
      onLeave: opts.onLeave,
    },
  });
  opts.build(tl);
  return tl.scrollTrigger ?? null;
}

/** 路由切换 / 卸载时清理所有 ScrollTrigger 实例（避免 memory leak）。 */
export function killAllScrollTriggers(): void {
  if (typeof window === "undefined") return;
  ScrollTrigger.getAll().forEach((st) => st.kill());
}
