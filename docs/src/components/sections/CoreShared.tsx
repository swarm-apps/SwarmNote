/**
 * Section 04 — 一份 Rust 核心，桌面与移动同源
 * 桌面（Tauri）⇄ swarmnote-core（Rust crate） ⇄ 移动（Expo + uniffi）
 * GSAP 滚动驱动 + 双向粒子流动画
 */
import { useEffect, useRef } from "react";
import { gsap } from "gsap";
import {
  ensureScrollTrigger,
  prefersReducedMotion,
} from "../../lib/scroll-utils";

export interface CoreSharedDict {
  h1: string;
  lede: string;
  leftLabel: string;
  leftStack: string;
  leftCall: string;
  centerLabel: string;
  centerStack: string;
  rightLabel: string;
  rightStack: string;
  rightCall: string;
  footnote: string;
}

interface Props {
  dict: CoreSharedDict;
}

const SVG_W = 960;
const SVG_H = 360;
const CY = SVG_H / 2;
const LEFT_X = 140;
const RIGHT_X = SVG_W - 140;
const CENTER_X = SVG_W / 2;

export default function CoreShared({ dict }: Props) {
  const rootRef = useRef<HTMLElement | null>(null);
  const svgRef = useRef<SVGSVGElement | null>(null);

  useEffect(() => {
    const root = rootRef.current;
    const svg = svgRef.current;
    if (!root || !svg) return;

    const ScrollTrigger = ensureScrollTrigger();
    const reduced = prefersReducedMotion();

    const ctx = gsap.context(() => {
      const eyebrow = root.querySelector("[data-cs-eyebrow]");
      const h2 = root.querySelector("[data-cs-h2]");
      const lede = root.querySelector("[data-cs-lede]");
      const blocks = root.querySelectorAll("[data-cs-block]");
      const corePulse = svg.querySelector("[data-core-pulse]");
      const arrowL = svg.querySelector("[data-arrow-l]");
      const arrowR = svg.querySelector("[data-arrow-r]");
      const particlesL = Array.from(
        svg.querySelectorAll<SVGCircleElement>("[data-particle-l]"),
      );
      const particlesR = Array.from(
        svg.querySelectorAll<SVGCircleElement>("[data-particle-r]"),
      );
      const footnote = root.querySelector("[data-cs-footnote]");

      if (reduced) {
        return;
      }

      gsap.set([eyebrow, h2, lede], { y: 24, autoAlpha: 0 });
      gsap.set(blocks, { y: 30, autoAlpha: 0 });
      gsap.set(footnote, { autoAlpha: 0 });
      gsap.set([arrowL, arrowR], { autoAlpha: 0 });
      gsap.set(corePulse, { transformOrigin: "center", scale: 0.5, autoAlpha: 0 });
      gsap.set([...particlesL, ...particlesR], { autoAlpha: 0 });

      gsap
        .timeline({
          scrollTrigger: {
            trigger: root,
            start: "top 65%",
            end: "top 10%",
            toggleActions: "play none none reverse",
          },
        })
        .to(eyebrow, { y: 0, autoAlpha: 1, duration: 0.5, ease: "power2.out" })
        .to(h2, { y: 0, autoAlpha: 1, duration: 0.6, ease: "power2.out" }, "-=0.3")
        .to(lede, { y: 0, autoAlpha: 1, duration: 0.5, ease: "power2.out" }, "-=0.4")
        .to(blocks, {
          y: 0,
          autoAlpha: 1,
          duration: 0.6,
          stagger: 0.12,
          ease: "power2.out",
        }, "-=0.2")
        .to(corePulse, {
          scale: 1,
          autoAlpha: 1,
          duration: 0.7,
          ease: "back.out(1.6)",
        }, "-=0.4")
        .to(
          [arrowL, arrowR],
          {
            autoAlpha: 1,
            duration: 0.7,
            ease: "power2.out",
            stagger: 0.1,
          },
          "-=0.4",
        )
        .to(footnote, { autoAlpha: 1, duration: 0.5 }, "-=0.2");

      // 持续：核心呼吸光晕
      gsap.to(corePulse, {
        scale: 1.08,
        repeat: -1,
        yoyo: true,
        duration: 1.6,
        ease: "sine.inOut",
      });

      // 持续：双向粒子流（仅当 section 接近视口）
      const flowTl = gsap.timeline({ repeat: -1, paused: true });
      const TRAVEL = 1.4;
      const buildParticle = (
        p: SVGCircleElement,
        from: number,
        to: number,
      ) => {
        const sub = gsap.timeline();
        sub.fromTo(
          p,
          { attr: { cx: from, cy: CY } },
          { attr: { cx: to, cy: CY }, duration: TRAVEL, ease: "power1.inOut" },
          0,
        );
        sub.fromTo(
          p,
          { autoAlpha: 0 },
          { autoAlpha: 0.85, duration: TRAVEL * 0.25, ease: "power1.out" },
          0,
        );
        sub.to(
          p,
          { autoAlpha: 0, duration: TRAVEL * 0.35, ease: "power1.in" },
          TRAVEL * 0.65,
        );
        return sub;
      };
      particlesL.forEach((p, i) => {
        flowTl.add(buildParticle(p, LEFT_X + 16, CENTER_X - 30), i * 0.45);
      });
      particlesR.forEach((p, i) => {
        flowTl.add(buildParticle(p, RIGHT_X - 16, CENTER_X + 30), i * 0.45 + 0.2);
      });

      ScrollTrigger.create({
        trigger: root,
        start: "top 80%",
        end: "bottom 20%",
        onEnter: () => flowTl.play(),
        onEnterBack: () => flowTl.play(),
        onLeave: () => flowTl.pause(),
        onLeaveBack: () => flowTl.pause(),
      });
    }, root);

    requestAnimationFrame(() => ScrollTrigger.refresh());

    return () => ctx.revert();
  }, []);

  // Path: 左端点 → 中央核心；中央核心 → 右端点
  const arrowL = `M ${LEFT_X + 30} ${CY} L ${CENTER_X - 40} ${CY}`;
  const arrowR = `M ${CENTER_X + 40} ${CY} L ${RIGHT_X - 30} ${CY}`;

  return (
    <section
      ref={rootRef}
      id="core-shared"
      className="relative overflow-hidden py-32 md:py-40"
    >
      <div className="max-w-6xl mx-auto px-6">
        <header className="text-center mb-12 md:mb-16 max-w-3xl mx-auto">
          <p
            data-cs-eyebrow
            className="text-xs font-mono uppercase tracking-[0.25em] text-primary mb-4"
          >
            Same core. Two surfaces.
          </p>
          <h2
            data-cs-h2
            className="text-4xl md:text-5xl font-semibold tracking-tight text-foreground mb-5"
          >
            {dict.h1}
          </h2>
          <p data-cs-lede className="text-base md:text-lg text-muted-foreground leading-relaxed">
            {dict.lede}
          </p>
        </header>

        {/* 桌面 ⇄ core ⇄ 移动 SVG 图 */}
        <div className="relative">
          <svg
            ref={svgRef}
            viewBox={`0 0 ${SVG_W} ${SVG_H}`}
            className="w-full h-auto"
            aria-hidden="true"
          >
            <defs>
              <radialGradient id="coreGlow" cx="50%" cy="50%" r="50%">
                <stop offset="0%" stopColor="var(--color-glow)" stopOpacity="0.55" />
                <stop offset="60%" stopColor="var(--color-primary)" stopOpacity="0.18" />
                <stop offset="100%" stopColor="var(--color-primary)" stopOpacity="0" />
              </radialGradient>
            </defs>

            {/* 中央核心呼吸光晕 */}
            <circle
              data-core-pulse
              cx={CENTER_X}
              cy={CY}
              r="80"
              fill="url(#coreGlow)"
            />

            {/* 中央 hex 核心 */}
            <g transform={`translate(${CENTER_X} ${CY})`}>
              <polygon
                points="46,0 23,39.84 -23,39.84 -46,0 -23,-39.84 23,-39.84"
                fill="var(--color-card)"
                stroke="var(--color-primary)"
                strokeWidth="1.5"
              />
              <text
                x="0"
                y="-4"
                textAnchor="middle"
                fill="var(--color-foreground)"
                fontSize="13"
                fontFamily="ui-monospace, monospace"
                fontWeight="600"
              >
                {dict.centerLabel}
              </text>
              <text
                x="0"
                y="14"
                textAnchor="middle"
                fill="var(--color-muted-foreground)"
                fontSize="10"
                fontFamily="ui-monospace, monospace"
              >
                {dict.centerStack}
              </text>
            </g>

            {/* 左端点（桌面） */}
            <g transform={`translate(${LEFT_X} ${CY})`}>
              <rect
                x="-46"
                y="-30"
                width="92"
                height="60"
                rx="8"
                fill="var(--color-card)"
                stroke="var(--color-border)"
                strokeWidth="1.2"
              />
              <text
                x="0"
                y="-6"
                textAnchor="middle"
                fill="var(--color-foreground)"
                fontSize="13"
                fontWeight="600"
              >
                {dict.leftLabel}
              </text>
              <text
                x="0"
                y="14"
                textAnchor="middle"
                fill="var(--color-muted-foreground)"
                fontSize="10"
                fontFamily="ui-monospace, monospace"
              >
                {dict.leftStack}
              </text>
            </g>

            {/* 右端点（移动） */}
            <g transform={`translate(${RIGHT_X} ${CY})`}>
              <rect
                x="-46"
                y="-30"
                width="92"
                height="60"
                rx="8"
                fill="var(--color-card)"
                stroke="var(--color-border)"
                strokeWidth="1.2"
              />
              <text
                x="0"
                y="-6"
                textAnchor="middle"
                fill="var(--color-foreground)"
                fontSize="13"
                fontWeight="600"
              >
                {dict.rightLabel}
              </text>
              <text
                x="0"
                y="14"
                textAnchor="middle"
                fill="var(--color-muted-foreground)"
                fontSize="10"
                fontFamily="ui-monospace, monospace"
              >
                {dict.rightStack}
              </text>
            </g>

            {/* 连接线 */}
            <path
              data-arrow-l
              d={arrowL}
              fill="none"
              stroke="var(--color-primary)"
              strokeOpacity="0.55"
              strokeWidth="1.5"
              strokeDasharray="4 3"
            />
            <path
              data-arrow-r
              d={arrowR}
              fill="none"
              stroke="var(--color-primary)"
              strokeOpacity="0.55"
              strokeWidth="1.5"
              strokeDasharray="4 3"
            />

            {/* 流动粒子 */}
            {[0, 1, 2].map((i) => (
              <circle
                key={`pl-${i}`}
                data-particle-l
                cx={LEFT_X + 16}
                cy={CY}
                r="3"
                fill="var(--color-glow)"
              />
            ))}
            {[0, 1, 2].map((i) => (
              <circle
                key={`pr-${i}`}
                data-particle-r
                cx={RIGHT_X - 16}
                cy={CY}
                r="3"
                fill="var(--color-glow)"
              />
            ))}

            {/* 调用方式标签 */}
            <text
              x={(LEFT_X + CENTER_X) / 2}
              y={CY - 14}
              textAnchor="middle"
              fill="var(--color-muted-foreground)"
              fontSize="11"
              fontFamily="ui-monospace, monospace"
            >
              {dict.leftCall}
            </text>
            <text
              x={(RIGHT_X + CENTER_X) / 2}
              y={CY - 14}
              textAnchor="middle"
              fill="var(--color-muted-foreground)"
              fontSize="11"
              fontFamily="ui-monospace, monospace"
            >
              {dict.rightCall}
            </text>
          </svg>
        </div>

        {/* 三段卡片解释 */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-5 mt-12 md:mt-16 max-w-5xl mx-auto">
          <article
            data-cs-block
            className="rounded-xl border border-border bg-card p-6 md:p-7"
          >
            <p className="text-xs font-mono uppercase tracking-widest text-primary mb-3">
              Desktop
            </p>
            <h3 className="text-lg font-semibold mb-2">{dict.leftLabel}</h3>
            <p className="text-sm text-muted-foreground mb-4">{dict.leftStack}</p>
            <code className="block rounded-md bg-muted/60 px-3 py-2 text-xs font-mono text-foreground/80 overflow-x-auto">
              {dict.leftCall}
            </code>
          </article>
          <article
            data-cs-block
            className="rounded-xl border border-primary/40 bg-card ring-1 ring-primary/15 p-6 md:p-7 relative"
          >
            <p className="text-xs font-mono uppercase tracking-widest text-primary mb-3">
              Core
            </p>
            <h3 className="text-lg font-semibold mb-2">{dict.centerLabel}</h3>
            <p className="text-sm text-muted-foreground mb-4">{dict.centerStack}</p>
            <ul className="text-xs font-mono text-muted-foreground space-y-1">
              <li>· CRDT (yrs)</li>
              <li>· libp2p networking</li>
              <li>· yrs-blocknote bridge</li>
            </ul>
          </article>
          <article
            data-cs-block
            className="rounded-xl border border-border bg-card p-6 md:p-7"
          >
            <p className="text-xs font-mono uppercase tracking-widest text-primary mb-3">
              Mobile
            </p>
            <h3 className="text-lg font-semibold mb-2">{dict.rightLabel}</h3>
            <p className="text-sm text-muted-foreground mb-4">{dict.rightStack}</p>
            <code className="block rounded-md bg-muted/60 px-3 py-2 text-xs font-mono text-foreground/80 overflow-x-auto">
              {dict.rightCall}
            </code>
          </article>
        </div>

        <p
          data-cs-footnote
          className="mt-10 text-center text-xs md:text-sm text-muted-foreground italic"
        >
          {dict.footnote}
        </p>
      </div>
    </section>
  );
}
