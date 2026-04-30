# SwarmNote 官网文案 — 中文（源语言）

> 本稿是 10 段叙事章节的中文文案稿骨架。每段含一句 H1 / 一段引子（≤ 60 字）/ 必要的 CTA 与微文案。
> 文案外化到 `docs/src/i18n/zh.json`，结构与本稿键一一对应；`content-en.md` 为对应翻译。

## 01 Hero

- **H1**：让笔记在你的设备群里自由流动
- **副标题**：本地优先 · P2P 同步 · CRDT 自动合并 · 桌面 + 移动同源
- **副标题（短版，移动端）**：你的笔记，蜂群里的花蜜
- **CTA primary**：免费下载
- **CTA secondary**：在 GitHub 上查看
- **微文案**：开源 · 无需账号 · 无需服务器

## 02 蜂群隐喻

- **H1**：为什么是 swarm？
- **导语**：把"属于你自己的多台设备"想象成一个蜂群——每台设备是一只蜜蜂，工作区是蜂巢，笔记像花蜜一样在蜂群间自由流动。
- **三句注解**：
  - 设备 = 蜜蜂：你的笔记本、手机、台式机各是蜂群里的一员
  - 工作区 = 蜂巢：所有设备共享同一片"地盘"
  - 笔记 = 花蜜：在 swarm 内部 P2P 流动，不经过任何中心服务器

## 03 四大支柱

- **H1**：为什么选 SwarmNote
- **支柱 1 / 本地优先**：笔记 = 普通 .md 文件夹，删了应用笔记还在
- **支柱 2 / P2P 同步**：libp2p 直连设备，无服务器、无订阅
- **支柱 3 / CRDT 自动合并**：基于 Yjs，离线编辑零冲突
- **支柱 4 / 全平台**：Windows · macOS · Linux · Android · iOS

## 04 同源跨平台 ★（叙事高潮）

- **H1**：一份 Rust 核心，桌面与移动同源
- **导语**：桌面端 (Tauri) 和移动端 (Expo) 通过 `swarmnote-core` Rust crate 共享同一份业务逻辑、CRDT 状态机和 P2P 网络栈。
- **左栏（Desktop）**：Tauri 2 + React 19 + invoke('cmd', args)
- **中央**：`swarmnote-core` Rust crate（Crab 图标 + 发光 + 一段 Rust 代码片段）
- **右栏（Mobile）**：Expo + React Native + uniffi.call() (JSI Turbo Module)
- **底注**：`@swarmnote/editor` git submodule 同时被两端 import，CodeMirror 6 编辑体验完全一致

## 05 设备入群

- **H1**：6 位配对码加入 swarm
- **导语**：在一台设备上生成 6 位数字，在另一台输入即配对。跨网络可用，零云端中转。
- **分步说明**：
  - 1. 在主设备打开"添加设备"
  - 2. 一段 6 位数字配对码出现
  - 3. 新设备输入这串数字
  - 4. swarm 完成扩展，笔记自动同步
- **副注**：局域网内设备由 mDNS 自动发现，无需手输配对码（开发中）

## 06 笔记 = 普通文件夹

- **H1**：你的笔记，永远在你手里
- **导语**：工作区就是任意本地文件夹。打开 VS Code、Obsidian、nvim 直接编辑 .md 文件，SwarmNote 监听变更并自动 reflow 进 Yjs，不丢历史不冲突。
- **代码示例**：
  ```
  my-notes/
  ├── .swarmnote/workspace.db
  ├── meeting-notes.md
  └── ideas/product-plan.md
  ```
- **副注**：能放进 Dropbox / OneDrive / Git 等任意现有文件同步方案做额外备份

## 07 与同类对比

- **H1**：选择 SwarmNote 的理由
- **对比维度**：数据存储 / 多设备同步 / 服务器依赖 / 离线协作合并 / 外部工具协同 / 开源
- **同类**：Obsidian · Notion · Logseq · SwarmNote
- **关键论据**：唯有 SwarmNote 同时具备「本地 Markdown + 内置 P2P + CRDT 字符级合并 + 完全开源 (MIT)」

## 08 下载安装

- **H1**：选择你的平台
- **副标题**：当前最新版本：v{version}（{publishedAt}）
- **平台卡片标题**：Windows · macOS · Linux · Android · iOS
- **Android 占位**：内测中 · 即将发布
- **iOS 占位**：开发完成 · 等待签名上架
- **副链接**：查看完整下载选项 → /download

## 09 Swarm 生态

- **H1**：Swarm 系列开源项目
- **导语**：所有项目共享 [`swarm-p2p-core`](https://github.com/yexiyue/swarm-p2p) 网络层
- **卡片**：
  - **SwarmDrop** — 去中心化文件传输（"跨网络版 LocalSend"）
  - **SwarmNote** — P2P 笔记同步（你正在看的这个）
  - **SwarmNote-RN** — SwarmNote 移动端
  - **swarm-p2p-core** — P2P 网络 SDK（libp2p 封装）

## 10 Footer

- **品牌段**：Built with Tauri · libp2p · CodeMirror 6 · Yjs
- **链接列**：
  - **产品**：下载 / 更新日志 / 路线图
  - **开发**：GitHub / 提交 Issue / 贡献指南
  - **法律**：隐私 / 许可证 (MIT)
  - **生态**：SwarmDrop / swarm-p2p-core
- **底栏**：© 2025 SwarmNote Contributors · MIT License
- **微文案**：本网站不收集任何用户数据 · No tracking · No telemetry

## 全站微文案池

- 顶部导航：特性 / 下载 / 生态 / GitHub
- 语言切换器：中文 ↔ English
- 浏览器主题：亮色（暂不支持暗色）
- "正在加载 Hero"：（不显示文字，只用骨架屏）
- "下载推荐你的平台"：徽标文字
- 404 页：这只蜜蜂飞错了方向 · 返回首页
