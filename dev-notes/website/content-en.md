# SwarmNote Website Copy — English (translated)

> Mirror structure of `content-zh.md`. Externalized to `docs/src/i18n/en.json`.
> Keep keys identical to zh; English copy must be standalone (not transliterated).

## 01 Hero

- **H1**: Your notes, swarming across your own devices
- **Subtitle**: Local-first · P2P sync · CRDT auto-merge · One Rust core for desktop and mobile
- **Subtitle (short, mobile)**: Notes that flow like nectar in your swarm
- **CTA primary**: Download free
- **CTA secondary**: View on GitHub
- **Microcopy**: Open source · No accounts · No servers

## 02 Why Swarm?

- **H1**: Why a swarm?
- **Lede**: Think of your devices as a swarm of bees: each device is a bee, the workspace is the hive, and notes flow like nectar between them.
- **Three notes**:
  - Devices = bees: your laptop, phone, and desktop are members of the swarm
  - Workspace = hive: shared territory across every device
  - Notes = nectar: flowing peer-to-peer, never through a central server

## 03 Four Pillars

- **H1**: Why SwarmNote
- **Pillar 1 / Local-first**: Notes are plain `.md` files. Uninstall the app — your notes stay.
- **Pillar 2 / P2P sync**: libp2p connects your devices directly. No server. No subscription.
- **Pillar 3 / CRDT auto-merge**: Powered by Yjs. Edit offline, no conflicts.
- **Pillar 4 / Cross-platform**: Windows · macOS · Linux · Android · iOS

## 04 One Rust Core ★ (narrative climax)

- **H1**: One Rust core, desktop and mobile
- **Lede**: Tauri desktop and Expo mobile share the same `swarmnote-core` Rust crate — same business logic, same CRDT state, same P2P stack.
- **Left (Desktop)**: Tauri 2 + React 19 · `invoke('cmd', args)`
- **Center**: `swarmnote-core` Rust crate (crab + glow + Rust snippet)
- **Right (Mobile)**: Expo + React Native · `uniffi.call()` (JSI Turbo Module)
- **Footnote**: `@swarmnote/editor` git submodule is imported on both sides — CodeMirror 6 editing experience is identical

## 05 Join the Swarm

- **H1**: Join with a 6-digit pairing code
- **Lede**: Generate a 6-digit code on one device, type it on another. Works across networks. No cloud relay.
- **Steps**:
  - 1. Open "Add device" on your primary
  - 2. A 6-digit code appears
  - 3. Type the code on the new device
  - 4. The swarm grows; notes sync automatically
- **Sidenote**: Devices on the same Wi-Fi are auto-discovered via mDNS (in development)

## 06 Notes = a folder

- **H1**: Your notes stay yours
- **Lede**: A workspace is just a local folder. Open `.md` files in VS Code, Obsidian, or nvim. SwarmNote watches and merges your edits back into Yjs — no history loss, no conflicts.
- **Code**:
  ```
  my-notes/
  ├── .swarmnote/workspace.db
  ├── meeting-notes.md
  └── ideas/product-plan.md
  ```
- **Sidenote**: Drop the workspace into Dropbox, OneDrive, or Git for additional backup

## 07 Compared to others

- **H1**: Why pick SwarmNote
- **Dimensions**: Storage / Multi-device sync / Server dependency / Offline merging / External tool support / Open source
- **Compared to**: Obsidian · Notion · Logseq · SwarmNote
- **Argument**: Only SwarmNote ships local Markdown + built-in P2P + character-level CRDT merge + fully open source (MIT)

## 08 Download

- **H1**: Pick your platform
- **Subtitle**: Latest release: v{version} ({publishedAt})
- **Platform titles**: Windows · macOS · Linux · Android · iOS
- **Android placeholder**: Beta in progress · Coming soon
- **iOS placeholder**: Built · Awaiting Apple Developer signing
- **Sublink**: See all download options → /en/download

## 09 The Swarm ecosystem

- **H1**: Swarm open-source family
- **Lede**: All projects share the [`swarm-p2p-core`](https://github.com/swarm-apps/swarm-p2p) network layer
- **Cards**:
  - **SwarmDrop** — Decentralized file transfer ("LocalSend across networks")
  - **SwarmNote** — P2P notes (you're here)
  - **SwarmNote-RN** — SwarmNote mobile
  - **swarm-p2p-core** — P2P SDK (libp2p wrapper)

## 10 Footer

- **Brand line**: Built with Tauri · libp2p · CodeMirror 6 · Yjs
- **Link columns**:
  - **Product**: Download / Changelog / Roadmap
  - **Develop**: GitHub / Issues / Contributing
  - **Legal**: Privacy / License (MIT)
  - **Ecosystem**: SwarmDrop / swarm-p2p-core
- **Bottom**: © 2025 SwarmNote Contributors · MIT License
- **Microcopy**: This website collects no user data · No tracking · No telemetry

## Site-wide microcopy pool

- Top nav: Features / Download / Ecosystem / GitHub
- Language switcher: 中文 ↔ English
- Browser theme: Light (dark mode not yet supported)
- "Loading Hero": (no text — skeleton only)
- "Recommended for your platform": badge text
- 404: This bee flew the wrong way · Back home
