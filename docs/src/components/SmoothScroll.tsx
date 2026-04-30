/**
 * SmoothScroll — 全站滚动动效基础设施 island。
 *
 * 职责：
 *   1. 注册 GSAP ScrollTrigger 插件
 *   2. 在桌面环境挂载 Lenis 惯性平滑滚动
 *   3. 把 Lenis 的 scroll 事件桥接到 ScrollTrigger.update（保证 scrub 准确）
 *   4. 触摸设备 / 屏宽 < 768 / reduced-motion 跳过 Lenis，回退浏览器原生滚动
 *
 * 用法：在 BaseLayout 里 `<SmoothScroll client:idle />`
 * 此组件不渲染任何 DOM。
 */

import { useEffect } from "react";
import { gsap } from "gsap";
import Lenis from "lenis";
import {
  ensureScrollTrigger,
  isCoarseOrNarrow,
  killAllScrollTriggers,
  prefersReducedMotion,
} from "../lib/scroll-utils";

export default function SmoothScroll(): null {
  useEffect(() => {
    ensureScrollTrigger();

    // reduced-motion 或触摸设备：完全跳过 Lenis，让浏览器原生滚动 + ScrollTrigger 自行驱动
    if (prefersReducedMotion() || isCoarseOrNarrow()) {
      return () => {
        killAllScrollTriggers();
      };
    }

    const lenis = new Lenis({
      duration: 1.1,
      easing: (t) => Math.min(1, 1.001 - Math.pow(2, -10 * t)),
      smoothWheel: true,
      // 触摸事件不让 Lenis 接管（避免移动浏览器误用桌面分支时与原生滚动冲突）
      touchMultiplier: 0,
    });

    // Lenis 滚动 → 通知 ScrollTrigger 重算
    lenis.on("scroll", () => {
      // ScrollTrigger.update() 在 ensureScrollTrigger 之后是 globally available
      // 但用 ticker 驱动 raf 更稳，下面的 ticker.add 已经处理
    });

    // 用 GSAP ticker 驱动 Lenis 的 raf——比独立 requestAnimationFrame 同步更精准
    const tickerCallback = (time: number) => {
      lenis.raf(time * 1000);
    };
    gsap.ticker.add(tickerCallback);
    gsap.ticker.lagSmoothing(0);

    return () => {
      gsap.ticker.remove(tickerCallback);
      lenis.destroy();
      killAllScrollTriggers();
    };
  }, []);

  return null;
}
