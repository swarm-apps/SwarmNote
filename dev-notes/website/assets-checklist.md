# SwarmNote 官网素材清单

> 本清单跟踪 `docs/public/` 与 `docs/src/` 下需要的图片、字体、3D 模型、OG 图等非代码素材。状态：✅ 已就位 / 🚧 进行中 / 📋 待办 / ↩ 复用

## 品牌资源

| 文件 | 用途 | 状态 | 来源 |
|---|---|---|---|
| `public/brand/logo.svg` | 顶导 / Hero 中央 / Favicon SVG | 📋 | 现有 `logo.png` 矢量化 |
| `public/brand/logo.png@2x.png` | OG 图 + 高 DPI fallback | 📋 | 现有 `logo.png`（512×512）|
| `public/brand/wordmark.svg` | 顶导左侧"SwarmNote" 字标 | 📋 | Inter Display Bold 渲染 + 蜂蜜金 |

## Hero 3D 蜂群

| 文件 | 用途 | 状态 | 备注 |
|---|---|---|---|
| ~~`public/brand/bee.glb`~~ | 蜜蜂 3D 模型 | ❌ 改用 procedural mesh | 见下方决策修订 |

**实施决策修订**：放弃 .glb 外部资源，改在 `HeroSwarm.tsx` 内用 Three.js 几何原语**程序化合成**蜜蜂（一个椭球身体 `EllipsoidGeometry` + 两片梯形翅膀 `PlaneGeometry` + 几条黑色横纹通过 vertex color 实现）。优势：

1. 零外部资源依赖，bundle 更小
2. 不需要建模 / Blender 工具链
3. 可在运行时按需调整面数（200 三角面预算可控）
4. 翅膀可单独驱动 (sin(t) 抖动)，比 .glb 静态模型更动感

`docs/src/components/HeroSwarm.tsx` 内自带工厂函数 `createBeeGeometry({ tris: 60, wingShake: true })`。

## Favicon 套

| 文件 | 用途 | 状态 |
|---|---|---|
| `public/favicon.svg` | 现代浏览器主 favicon | 📋 |
| `public/favicon-32x32.png` | 旧浏览器 fallback | 📋 |
| `public/favicon-16x16.png` | 同上 | 📋 |
| `public/apple-touch-icon.png` (180×180) | iOS / iPadOS | 📋 |
| `public/android-chrome-192x192.png` | Android home screen | 📋 |
| `public/site.webmanifest` | PWA manifest（可选） | 📋 |

生成方式：从 `public/brand/logo.svg` 用 [realfavicongenerator.net](https://realfavicongenerator.net) 一键导出全套，或本地用 `sharp` 脚本。

## OG / Twitter Card

每张 1200×630 PNG。中文与英文版本独立。

| 文件 | 页面 | 状态 |
|---|---|---|
| `public/og/home-zh.png` | / | 📋 |
| `public/og/home-en.png` | /en/ | 📋 |
| `public/og/download-zh.png` | /download | 📋 |
| `public/og/download-en.png` | /en/download | 📋 |
| `public/og/changelog-zh.png` | /changelog | 📋 |
| `public/og/changelog-en.png` | /en/changelog | 📋 |

设计模板：暖白底（#FDFCFA）+ 左侧 logo + 中央 H1（Inter Display 72px 蜂蜜金）+ 右下"swarmnote.app"。可用 [og-img](https://github.com/vercel/og) 编程生成。

## 字体（自托管）

放 `public/fonts/`，BaseLayout 内 `<link rel="preload" as="font" crossorigin>` 引入。

| 字体 | 子集 | 来源 | 状态 |
|---|---|---|---|
| Inter Variable (woff2) | Latin + Latin-ext | `@fontsource-variable/inter` | 📋 |
| Inter Display Variable (woff2) | Latin + Latin-ext | Google Fonts / Rsms | 📋 |
| JetBrains Mono Variable (woff2) | Latin + Box drawing | `@fontsource-variable/jetbrains-mono` | 📋 |

**子集化建议**：用 `pyftsubset` 或 `glyphhanger` 进一步裁剪到本站实际使用字符；中文 fallback 到系统字（PingFang SC / Microsoft YaHei / Noto Sans CJK），不内嵌中文 woff2（会让 bundle 暴增 5MB+）。

## 截图素材（Pillars / Folder / Compare 章节用）

| 文件 | 用途 | 状态 |
|---|---|---|
| `public/screenshots/desktop-editor.png` | 桌面端 BlockNote/CM6 编辑器主界面 | 📋（在主仓 dev 模式下截图）|
| `public/screenshots/desktop-sidebar.png` | 文件树 + 编辑器 split | 📋 |
| `public/screenshots/mobile-editor.png` | 移动端 WebView 编辑器 | 📋（swarmnote-mobile dev build 截图）|
| `public/screenshots/mobile-pairing.png` | 移动端 6 位配对码界面 | 📋 |
| `public/screenshots/finder-folder.png` | macOS Finder 中的工作区目录 | 📋（手工截图）|
| `public/screenshots/vscode-md.png` | VS Code 编辑同一份 .md 文件 | 📋 |

所有截图建议提供 1× 与 2× 两份，Astro `<Image />` 自动 webp/avif 转换。

## 其他

| 文件 | 用途 | 状态 |
|---|---|---|
| `public/CNAME` | GitHub Pages custom domain（写 `swarmnote.app`） | 📋 |
| `public/robots.txt` | SEO 爬虫规则 | 📋 |
| `public/sitemap-index.xml` | 由 `@astrojs/sitemap` 自动生成（不手写） | 自动 |

## 复用主仓的素材

| 文件 | 复用路径 | 备注 |
|---|---|---|
| 主仓 `logo.png` | → `docs/public/brand/logo.png` | 可直接 cp |
| 主仓 `app-icon.png` (src-tauri/icons/) | 参考 | 不直接复用 |
