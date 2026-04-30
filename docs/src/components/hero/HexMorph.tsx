/**
 * HexMorph — Hero 主视觉。
 *
 * 灵感：anime.js hero 的"滚动驱动连续 morph"模式。
 * 5 段 morph + GSAP ScrollTrigger pin (3 viewport) + scrub。
 *
 *   Stage 1 (0–20%)   单个发光六角形居中                                   "Local-first."
 *   Stage 2 (20–40%)  中心 + 6 周围 hex 扇出 + dotted 连线 + lucide icon  "P2P sync."
 *   Stage 3 (40–60%)  连线上流动光粒子                                    "CRDT auto-merge."
 *   Stage 4 (60–80%)  中心显示 swarmnote-core + 整体微缩 stagger          "Cross-platform."
 *   Stage 5 (80–100%) hex 转深色线框 + 标签线引出模块名 (CAD 蓝图)        "Open-source. Yours."
 *
 * 架构：每个 ring hex 及其内部 icon + 外部标签包在同一 <g data-ring="i">，整体 transform。
 * Lucide icon 直接内嵌 path（viewBox 24x24, stroke-based）—— 零依赖。
 */

import { useEffect, useRef, useState } from "react";
import { gsap } from "gsap";
import { ensureScrollTrigger, prefersReducedMotion } from "../../lib/scroll-utils";
import { withBase } from "../../lib/url";

// === 几何参数 ===
const SVG_W = 900;
const SVG_H = 540;
const CENTER_X = SVG_W / 2;
const CENTER_Y = SVG_H / 2;
const ORBIT_R = 170;
const HEX_SIZE = 48;
const LABEL_OFFSET = 64;

function hexPointsLocal(size: number): string {
  const angles = [30, 90, 150, 210, 270, 330];
  return angles
    .map((deg) => {
      const r = (deg * Math.PI) / 180;
      return `${size * Math.cos(r)},${size * Math.sin(r)}`;
    })
    .join(" ");
}

interface RingPos {
  x: number;
  y: number;
}

function ringPositions(): RingPos[] {
  return Array.from({ length: 6 }, (_, i) => {
    const angleDeg = i * 60 - 90;
    const r = (angleDeg * Math.PI) / 180;
    return {
      x: CENTER_X + ORBIT_R * Math.cos(r),
      y: CENTER_Y + ORBIT_R * Math.sin(r),
    };
  });
}

interface ModuleMeta {
  id: string;
  /** Lucide icon path data (viewBox 0 0 24 24, stroke-based) */
  iconPath: ReadonlyArray<{ d?: string; type?: "circle" | "rect"; cx?: number; cy?: number; r?: number; x?: number; y?: number; w?: number; h?: number; rx?: number }>;
  shortLabel: string;
}

// 内嵌 lucide-static path data。viewBox 0 0 24 24, stroke="currentColor" stroke-width=2 fill=none.
const MODULES: ModuleMeta[] = [
  {
    id: "workspace",
    // lucide:folder
    iconPath: [
      { d: "M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.93a2 2 0 0 1-1.66-.9l-.82-1.2A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" },
    ],
    shortLabel: "workspace",
  },
  {
    id: "documents",
    // lucide:file-text
    iconPath: [
      { d: "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" },
      { d: "M14 2v6h6" },
      { d: "M16 13H8" },
      { d: "M16 17H8" },
      { d: "M10 9H8" },
    ],
    shortLabel: "documents",
  },
  {
    id: "yjs",
    // lucide:git-merge
    iconPath: [
      { type: "circle", cx: 18, cy: 18, r: 3 },
      { type: "circle", cx: 6, cy: 6, r: 3 },
      { d: "M6 21V9a9 9 0 0 0 9 9" },
    ],
    shortLabel: "yjs CRDT",
  },
  {
    id: "p2p",
    // lucide:network
    iconPath: [
      { type: "rect", x: 16, y: 16, w: 6, h: 6, rx: 1 },
      { type: "rect", x: 2, y: 16, w: 6, h: 6, rx: 1 },
      { type: "rect", x: 9, y: 2, w: 6, h: 6, rx: 1 },
      { d: "M5 16v-3a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v3" },
      { d: "M12 12V8" },
    ],
    shortLabel: "p2p net",
  },
  {
    id: "identity",
    // lucide:key-round
    iconPath: [
      { d: "M2 18a4 4 0 0 1 4-4h2a4 4 0 0 1 4 4v2a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2Z" },
      { type: "circle", cx: 16, cy: 9, r: 7 },
      { d: "m18 6-1.5-1.5" },
    ],
    shortLabel: "identity",
  },
  {
    id: "pairing",
    // lucide:zap (闪电——快速配对的隐喻)
    iconPath: [
      { d: "M13 2 3 14h9l-1 8 10-12h-9l1-8z" },
    ],
    shortLabel: "pairing",
  },
];

const TAGLINES = [
  { h1: "Local-first.", sub: "你的笔记 = 普通 .md 文件夹" },
  { h1: "P2P sync.", sub: "设备直连，无需服务器" },
  { h1: "CRDT auto-merge.", sub: "字符级合并，离线零冲突" },
  { h1: "Cross-platform.", sub: "桌面 + 移动同源 Rust 核心" },
  { h1: "Open-source. Yours.", sub: "MIT · 你的笔记永远属于你" },
];

/** 渲染单个 lucide icon（局部坐标 0,0 为中心，scale 控制大小）。 */
function LucideIcon({
  paths,
  scale = 1.4,
  color = "#27211C",
}: {
  paths: ModuleMeta["iconPath"];
  scale?: number;
  color?: string;
}) {
  // viewBox 24x24 → 中心是 (12, 12)。translate(-12, -12) 让中心对齐 group origin
  return (
    <g
      transform={`scale(${scale}) translate(-12, -12)`}
      stroke={color}
      strokeWidth={1.6}
      fill="none"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      {paths.map((p, i) => {
        if (p.type === "circle") {
          return <circle key={i} cx={p.cx} cy={p.cy} r={p.r} />;
        }
        if (p.type === "rect") {
          return <rect key={i} x={p.x} y={p.y} width={p.w} height={p.h} rx={p.rx} ry={p.rx} />;
        }
        return <path key={i} d={p.d} />;
      })}
    </g>
  );
}

export default function HexMorph(): React.ReactElement {
  const containerRef = useRef<HTMLDivElement>(null);
  const [stage, setStage] = useState(0);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const reducedMotion = prefersReducedMotion();
    const ScrollTriggerCls = ensureScrollTrigger();

    const centerHex = container.querySelector<SVGPolygonElement>("[data-element='center-hex']");
    const centerLogo = container.querySelector<SVGImageElement>("[data-element='center-logo']");
    const centerCore = container.querySelector<SVGGElement>("[data-element='center-core']");
    const centerLabel = container.querySelector<SVGGElement>("[data-element='center-label']");
    const ringGroups = Array.from(container.querySelectorAll<SVGGElement>("[data-ring]"));
    const ringHexes = ringGroups.map((g) => g.querySelector<SVGPolygonElement>("[data-element='ring-hex']"));
    const ringIcons = ringGroups.map((g) => g.querySelector<SVGGElement>("[data-element='ring-icon']"));
    const ringLabels = ringGroups.map((g) => g.querySelector<SVGGElement>("[data-element='ring-label']"));
    const ringConnects = Array.from(container.querySelectorAll<SVGLineElement>("[data-element='ring-connect']"));
    const ringFlows = Array.from(container.querySelectorAll<SVGCircleElement>("[data-element='ring-flow']"));
    const progressFill = container.querySelector<HTMLDivElement>("[data-element='progress-fill']");

    const positions = ringPositions();

    // === 初始 ===
    if (centerHex) {
      gsap.set(centerHex, {
        attr: { fill: "#C99816", stroke: "rgba(122, 92, 14, 0.55)", "stroke-width": 1.5 },
        transformOrigin: "center",
        scale: 1,
        opacity: 1,
      });
    }
    if (centerLogo) {
      gsap.set(centerLogo, { transformOrigin: "center", opacity: 1, scale: 1 });
    }
    if (centerCore) gsap.set(centerCore, { opacity: 0 });
    if (centerLabel) gsap.set(centerLabel, { opacity: 0 });

    // 使用 attr.transform 显式设置 SVG transform 字符串（避免 GSAP 的 x/y 与 SVG inline transform 冲突）
    ringGroups.forEach((g) => {
      gsap.set(g, { attr: { transform: `translate(${CENTER_X} ${CENTER_Y})` }, opacity: 0 });
    });
    ringHexes.forEach((h) => {
      if (h)
        gsap.set(h, {
          attr: {
            fill: "rgba(201, 152, 22, 0.18)",
            stroke: "rgba(122, 92, 14, 0.45)",
            "stroke-width": 1.2,
          },
        });
    });
    ringIcons.forEach((ic) => ic && gsap.set(ic, { opacity: 0 }));
    ringLabels.forEach((l) => l && gsap.set(l, { opacity: 0 }));
    gsap.set(ringConnects, { opacity: 0 });
    gsap.set(ringFlows, { opacity: 0 });

    if (reducedMotion) {
      ringGroups.forEach((g, i) => {
        const pos = positions[i]!;
        gsap.set(g, { attr: { transform: `translate(${pos.x} ${pos.y})` }, opacity: 1 });
      });
      ringHexes.forEach((h) => {
        if (h)
          gsap.set(h, {
            attr: { fill: "transparent", stroke: "#27211C", "stroke-width": 1.5 },
          });
      });
      if (centerHex)
        gsap.set(centerHex, {
          attr: { fill: "transparent", stroke: "#27211C", "stroke-width": 1.5 },
        });
      if (centerLogo) gsap.set(centerLogo, { opacity: 0 });
      ringLabels.forEach((l) => l && gsap.set(l, { opacity: 1 }));
      if (centerLabel) gsap.set(centerLabel, { opacity: 1 });
      gsap.set(ringConnects, { opacity: 0.4 });
      setStage(4);
      return;
    }

    // === Timeline (pin 3 viewport, scrub) ===
    const tl = gsap.timeline({
      scrollTrigger: {
        trigger: container,
        start: "top top",
        end: "+=300%",
        pin: true,
        scrub: 0.6,
        anticipatePin: 1,
        onUpdate: (self) => {
          const p = self.progress;
          const idx = Math.min(4, Math.floor(p * 5));
          setStage(idx);
          if (progressFill) progressFill.style.width = `${p * 100}%`;
        },
      },
    });

    // ---- Stage 1 → 2 (0-20%): ring fade-in & 移到位置 + icon 显示 ----
    tl.to(centerHex, { scale: 0.85, duration: 0.15 }, 0);
    // 中央 logo：仅 stage 1 显眼，进入 stage 2 时缩小淡出，让位给 ring
    if (centerLogo) {
      tl.to(
        centerLogo,
        { opacity: 0, scale: 0.6, duration: 0.18, ease: "power2.in" },
        0.06,
      );
    }
    ringGroups.forEach((g, i) => {
      const pos = positions[i]!;
      tl.to(
        g,
        {
          attr: { transform: `translate(${pos.x} ${pos.y})` },
          opacity: 1,
          duration: 0.18,
          ease: "power3.out",
        },
        0.05 + i * 0.015,
      );
    });
    tl.to(ringConnects, { opacity: 0.5, duration: 0.12 }, 0.18);
    ringIcons.forEach((ic, i) => {
      if (ic) tl.to(ic, { opacity: 1, duration: 0.12 }, 0.22 + i * 0.01);
    });

    // ---- Stage 3 (40-60%): 流动光粒子 ----
    tl.to(ringFlows, { opacity: 1, duration: 0.08 }, 0.4);
    ringFlows.forEach((flow, i) => {
      const pos = positions[i]!;
      tl.fromTo(
        flow,
        { attr: { cx: CENTER_X, cy: CENTER_Y } },
        {
          attr: { cx: pos.x, cy: pos.y },
          duration: 0.16,
          ease: "power1.inOut",
          repeat: 1,
          yoyo: true,
        },
        0.42 + i * 0.015,
      );
    });

    // ---- Stage 4 (60-80%): 中央 swarmnote-core 显示 ----
    tl.to(ringFlows, { opacity: 0, duration: 0.08 }, 0.62);
    if (centerCore) tl.to(centerCore, { opacity: 1, duration: 0.15 }, 0.65);

    // ---- Stage 5 (80-100%): 转 CAD 线框 + 标签引线 ----
    tl.to(
      centerHex,
      { attr: { fill: "transparent", stroke: "#27211C", "stroke-width": 1.5 }, duration: 0.18 },
      0.8,
    );
    ringHexes.forEach((h) => {
      if (h)
        tl.to(
          h,
          { attr: { fill: "transparent", stroke: "#27211C", "stroke-width": 1.5 }, duration: 0.18 },
          0.8,
        );
    });
    if (centerCore) tl.to(centerCore, { opacity: 0, duration: 0.12 }, 0.82);
    ringIcons.forEach((ic, i) => {
      if (ic) tl.to(ic, { opacity: 0.4, duration: 0.12 }, 0.82 + i * 0.005);
    });
    ringLabels.forEach((l, i) => {
      if (l) tl.to(l, { opacity: 1, duration: 0.15 }, 0.85 + i * 0.015);
    });
    if (centerLabel) tl.to(centerLabel, { opacity: 1, duration: 0.15 }, 0.9);

    return () => {
      tl.scrollTrigger?.kill();
      tl.kill();
    };
  }, []);

  const positions = ringPositions();
  const currentTagline = TAGLINES[stage] ?? TAGLINES[0]!;

  return (
    <div ref={containerRef} className="relative w-full min-h-screen bg-background overflow-hidden">
      {/* 背景蜂巢极淡 pattern */}
      <svg className="absolute inset-0 w-full h-full pointer-events-none" aria-hidden="true" style={{ zIndex: 0 }}>
        <defs>
          <pattern id="hex-bg-pattern" x="0" y="0" width="60" height="52" patternUnits="userSpaceOnUse">
            <polygon points="30,2 56,17 56,47 30,62 4,47 4,17" fill="none" stroke="rgba(201, 152, 22, 0.06)" strokeWidth="1" />
          </pattern>
        </defs>
        <rect x="0" y="0" width="100%" height="100%" fill="url(#hex-bg-pattern)" />
      </svg>

      {/* 顶部 cycling tagline */}
      <div className="absolute top-16 left-0 right-0 px-6 max-w-5xl mx-auto text-center" style={{ zIndex: 20 }}>
        <a
          href="https://github.com/yexiyue/SwarmNote"
          className="inline-flex items-center gap-2 px-3 py-1 text-xs font-medium text-muted-foreground border border-border rounded-full bg-card/90 backdrop-blur hover:text-primary hover:border-primary transition-colors"
        >
          <span className="size-1.5 rounded-full bg-primary"></span>
          开源 · 无需账号 · 无需服务器
        </a>
        <h1
          key={`h1-${stage}`}
          className="mt-4 text-3xl sm:text-4xl md:text-5xl font-bold tracking-tight font-display max-w-3xl mx-auto"
          style={{ animation: "heroFadeIn 0.5s ease-out both" }}
        >
          {currentTagline.h1}
        </h1>
        <p
          key={`sub-${stage}`}
          className="mt-3 text-base md:text-lg text-muted-foreground max-w-2xl mx-auto"
          style={{ animation: "heroFadeIn 0.5s ease-out 0.06s both" }}
        >
          {currentTagline.sub}
        </p>
      </div>

      {/* 中央 SVG 舞台 */}
      <div className="absolute inset-0 flex items-center justify-center px-6" style={{ zIndex: 10 }}>
        <svg
          viewBox={`0 0 ${SVG_W} ${SVG_H}`}
          className="w-full max-w-275 h-auto"
          style={{ maxHeight: "60vh" }}
          aria-hidden="true"
        >
          {/* 6 条 dotted 连线 */}
          {positions.map((pos, i) => (
            <line
              key={`connect-${i}`}
              data-element="ring-connect"
              x1={CENTER_X}
              y1={CENTER_Y}
              x2={pos.x}
              y2={pos.y}
              stroke="rgba(120, 110, 100, 0.5)"
              strokeWidth="1"
              strokeDasharray="3 4"
            />
          ))}

          {/* 6 个流动光粒子 */}
          {positions.map((_, i) => (
            <circle
              key={`flow-${i}`}
              data-element="ring-flow"
              cx={CENTER_X}
              cy={CENTER_Y}
              r="4"
              fill="#DBA81E"
              opacity="0"
            />
          ))}

          {/* 6 个 ring group：每组含 hex + icon + label，整体 transform */}
          {MODULES.map((mod, i) => {
            const pos = positions[i]!;
            const dx = pos.x - CENTER_X;
            const dy = pos.y - CENTER_Y;
            const dist = Math.sqrt(dx * dx + dy * dy);
            // 默认径向外推；0° (top, i=0) 与 180° (bottom, i=3) 特殊处理水平偏移避免溢出 SVG 顶/底
            const isTop = i === 0;
            const isBottom = i === 3;
            let ux: number;
            let uy: number;
            if (isTop) {
              ux = 0.92; // 标签放右侧
              uy = -0.4; // 略上抬
            } else if (isBottom) {
              ux = 0.92;
              uy = 0.4;
            } else {
              ux = dx / dist;
              uy = dy / dist;
            }
            const labelStartX = ux * HEX_SIZE * 0.95;
            const labelStartY = uy * HEX_SIZE * 0.95;
            const labelEndX = ux * (HEX_SIZE + LABEL_OFFSET);
            const labelEndY = uy * (HEX_SIZE + LABEL_OFFSET);
            const textX = ux * (HEX_SIZE + LABEL_OFFSET + 6);
            const textY = uy * (HEX_SIZE + LABEL_OFFSET + 6);

            return (
              <g key={`ring-${i}`} data-ring={i} transform={`translate(${CENTER_X}, ${CENTER_Y})`}>
                <polygon data-element="ring-hex" points={hexPointsLocal(HEX_SIZE)} />
                <g data-element="ring-icon" opacity="0">
                  <LucideIcon paths={mod.iconPath} scale={1.4} color="#27211C" />
                </g>
                <g data-element="ring-label" opacity="0">
                  <line
                    x1={labelStartX}
                    y1={labelStartY}
                    x2={labelEndX}
                    y2={labelEndY}
                    stroke="#27211C"
                    strokeWidth="0.8"
                  />
                  <text
                    x={textX}
                    y={textY}
                    textAnchor={ux > 0.3 ? "start" : ux < -0.3 ? "end" : "middle"}
                    fontFamily="ui-monospace, monospace"
                    fontSize="11"
                    fontWeight="600"
                    fill="#27211C"
                    dominantBaseline="middle"
                  >
                    {mod.shortLabel}
                  </text>
                </g>
              </g>
            );
          })}

          {/* 中央 hex */}
          <polygon
            data-element="center-hex"
            points={hexPointsLocal(HEX_SIZE * 1.15)
              .split(" ")
              .map((p) => {
                const [x, y] = p.split(",").map(Number);
                return `${(x ?? 0) + CENTER_X},${(y ?? 0) + CENTER_Y}`;
              })
              .join(" ")}
            fill="#C99816"
            stroke="rgba(122, 92, 14, 0.55)"
            strokeWidth="1.5"
          />

          {/* 品牌 logo（stage 1 visible，进入 stage 2 时淡出） */}
          <image
            data-element="center-logo"
            href={withBase("/logo.png")}
            x={CENTER_X - 44}
            y={CENTER_Y - 44}
            width="88"
            height="88"
            preserveAspectRatio="xMidYMid meet"
            pointerEvents="none"
            style={{ filter: "drop-shadow(0 1px 2px rgba(0,0,0,0.18))" }}
          />

          {/* 中心 stage 4 标识：lucide hexagon icon (蜂巢) + swarmnote-core 文字 */}
          <g data-element="center-core" opacity="0">
            <g transform={`translate(${CENTER_X}, ${CENTER_Y - 8})`}>
              <LucideIcon
                paths={[{ d: "M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" }]}
                scale={1.0}
                color="#1F1605"
              />
            </g>
            <text
              x={CENTER_X}
              y={CENTER_Y + 18}
              textAnchor="middle"
              fontFamily="ui-monospace, monospace"
              fontSize="9"
              fontWeight="600"
              fill="#1F1605"
              letterSpacing="0.5"
            >
              swarmnote-core
            </text>
          </g>

          {/* 中心 stage 5 标签（线框 hex 内的蜂蜜金文字） */}
          <g data-element="center-label" opacity="0">
            <text
              x={CENTER_X}
              y={CENTER_Y + 5}
              textAnchor="middle"
              fontFamily="ui-monospace, monospace"
              fontSize="13"
              fontWeight="700"
              fill="#C99816"
              letterSpacing="0.5"
            >
              swarmnote-core
            </text>
          </g>
        </svg>
      </div>

      {/* CTA（hero 底部居中） */}
      <div className="absolute bottom-12 left-0 right-0 px-6" style={{ zIndex: 20 }}>
        <div className="flex flex-wrap items-center justify-center gap-3">
          <a
            href={withBase("/download")}
            className="px-5 py-2.5 rounded-lg bg-primary text-primary-foreground font-medium shadow-lg shadow-primary/20 hover:opacity-90 transition-opacity"
          >
            免费下载
          </a>
          <a
            href="https://github.com/yexiyue/SwarmNote"
            className="px-5 py-2.5 rounded-lg border border-border bg-background/80 backdrop-blur text-foreground hover:bg-accent transition-colors"
          >
            在 GitHub 上查看
          </a>
        </div>
      </div>

      {/* 滚动进度条（右下角浮动，类 anime.js 模式） */}
      <div
        className="absolute bottom-8 right-8 flex items-center gap-3 px-3 py-2 rounded-md bg-card/85 backdrop-blur border border-border"
        style={{ zIndex: 20 }}
      >
        <span className="text-[11px] font-mono text-muted-foreground tracking-wider">
          {stage + 1}/5
        </span>
        <div className="relative w-32 h-0.5 bg-border/70 rounded-full overflow-hidden">
          <div
            data-element="progress-fill"
            className="absolute inset-y-0 left-0 bg-primary"
            style={{ width: "0%" }}
          />
        </div>
        <span className="text-[11px] font-mono text-muted-foreground tracking-wider">
          scroll
        </span>
      </div>

      <style>{`
        @keyframes heroFadeIn {
          from { opacity: 0; transform: translateY(8px); }
          to { opacity: 1; transform: translateY(0); }
        }
      `}</style>
    </div>
  );
}
