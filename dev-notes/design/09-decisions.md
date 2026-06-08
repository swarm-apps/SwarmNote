# 设计决策记录

## 架构决策

| 决策 | 选择 | 参考 | 理由 |
|------|------|------|------|
| 文档格式 | Markdown (.md) 文件 | Obsidian | 用户可用任何编辑器打开，数据不锁死在应用内 |
| 编辑器 | BlockNote | Notion 风格 | Block-based，内置 yjs 协作支持，Markdown 导入导出 |
| 资源文件组织 | 同名目录（文档名/） | Obsidian | 直觉，文件管理器直接可见 |
| 存储分层 | .md 是真相源，db 是索引 | Git (.git/) | db 丢失可从文件重建，数据安全性高 |
| 身份与工作区分离 | 身份全局 (~/.swarmnote/)，工作区跟目录走 | Git (全局 config vs .git/) | 一台设备一个身份，工作区可自由移动 |
| 工作区发现 | 向上搜索 .swarmnote/ | Git (.git/ 搜索) | 用户打开子目录也能找到工作区 |
| CRDT 方案 | 仅前端 yjs，Rust 透传二进制 | — | 减少 yrs 依赖，降低维护成本 |

## 同步决策

| 决策 | 选择 | 参考 | 理由 |
|------|------|------|------|
| 同步架构 | 三级（yjs / FastCDC / 全量） | — | 不同数据类型用最适合的策略 |
| .md 文件同步 | blake3 + FastCDC 分块 | rsync, Restic | 内容分块，插入不影响其他 chunk，增量传输 |
| 资源文件同步 | mtime + blake3 hash，全量传输 | Syncthing | 资源文件通常整体替换，分块无意义 |
| 实时协作 | 两端都打开时自动升级为 yjs | — | 常态用文件级同步，按需升级为 CRDT |
| 外部编辑支持 | notify crate fs watcher | Obsidian | 检测外部修改，触发重新分块和同步 |
| 冲突处理 | 生成 .conflict 副本 | Dropbox, Syncthing | P2P 无法自动仲裁，交给用户 |

## 权限决策

| 决策 | 选择 | 参考 | 理由 |
|------|------|------|------|
| 角色数量 | 3级（Owner/Editor/Reader） | — | 个人/小团队笔记不需要 Commenter，减少密钥体系复杂度 |
| 不设 Commenter | 移除评论系统 | — | 评论对笔记场景优先级低，Obsidian 也没有评论功能 |
| 不设 Manager 角色 | — | 飞书有但我们不做 | MVP 阶段单 Owner 足够，多管理员引入复杂性 |
| 权限继承策略 | 向下继承 + 可覆盖 + 最高权限胜出 | Notion | 灵活但不复杂，避免 Google Docs 的强制继承限制 |
| 权限执行方式 | 密码学（密钥分发 + 签名验证） | Mega.nz、E2E 加密实践 | P2P 无服务器，唯一可行方案 |
| 链接分享安全 | URL fragment 嵌密钥 + 可选密码 | Mega.nz（fragment）、语雀（密码） | DHT 不存明文密钥，密码是额外保护层 |
| 链接有效期 | 支持自定义 | 腾讯文档（永久/7天/1天） | 必要的安全控制 |
| 权限撤销 | 停止同步 + 可选密钥轮换 | Google Docs（移除后保留已有） | 平衡安全性和实现复杂度 |
| Owner 模型 | 单 Owner，可转让 | Google Docs | 避免多 Owner 冲突，P2P 下难以仲裁 |

## E2E 加密/分享/权限 v1 决策（2026-06 定稿）

> 以下决策取代上方「权限决策」表中关于角色数量、密钥存储、撤销语义的纸面假设。完整设计见 [08-e2e-encryption.md](08-e2e-encryption.md) / [04-permissions.md](04-permissions.md) / [05-sharing.md](05-sharing.md) / [11-threat-model.md](11-threat-model.md)。

| 决策 | 选择 | 参考 | 理由 |
|------|------|------|------|
| 加密范围 | 仅传输加密（本地写明文 .md）| Tresorit、p2panda Data Encryption | 保住 folder-is-truth；丢设备交给 OS 全盘加密 |
| 加密主体 | **设备**（per-device Ed25519），非用户 | — | 无账号系统，唯一稳定密码学锚点是设备 |
| 密钥存储 | OS keychain（现有），非 Stronghold | `identity.rs` 已用 | 主体是设备 + 不防丢设备，Stronghold 前提不成立 |
| 群密钥方案 | per-workspace 对称 key + X25519 Lockbox + lazy 轮换 | Jazz/cojson、SecSync | 业界共识；MLS/BeeKEM 对 2-3 人 overkill |
| X25519 来源 | 从设备 Ed25519 复用派生（单层，钉死 clamp）| libsodium、GNUnet | 配对只换 PeerId 即可算对端公钥，零额外分发 |
| key commitment | 必加（HKDF-extra-output + 常量时间比对）| USENIX'21 partitioning oracle | 链接分享是攻击靶心，裸 AEAD 非 key-committing |
| 角色 | v1 两级 Owner/Collaborator | Obsidian Sync | 真 Reader 需逐 update 签名，后置 v2（已铺两把独立 key + 携带签名，v2 仅开开关）|
| 权限表 | 签名操作链（append-only `permission_ops`）| Matrix auth chain、Jazz | 防伪造提权，无中心可离线判定 |
| 撤销语义 | lazy re-encryption（被移除者旧数据永久可读）| Google Docs、Keyhive | CRDT 必留历史 key，FS 无意义；别承诺「踢人即焚」|
| 链接邀请传输 | 上 DHT + Owner 签名 + custom validator + HMAC key | Mega.nz | 支持双方离线异步兑换；#secret 永不上 DHT |
| 链接 max_uses | 不做强制（保留列不承诺）| — | 无中心计数不可靠，避免安全错觉；撤销靠 key rotation |
| 加密通道 | 只加密 GossipSub 广播；RR 同步靠 Noise | — | gossip 是真正裸奔点，收窄实现面 |

## 开放问题

1. **密钥备份/恢复**：用户丢失全部设备后如何恢复工作区密钥？助记词？加密导出文件？（v1 暂不做，需规划）
2. **多设备 UI 归组**：密码学上主体是设备（每设备一份 Lockbox），但用户心智是"我和协作者们"。UI 是否把同一人多 PeerId 聚合展示？可信依据是什么（无账号，只能手动标注/配对时自声明）？
3. **WorkspaceOpened 派生标识匹配**：`CtrlMessage::WorkspaceOpened` 改派生标识后，接收方如何判断"这是我也有的工作区"——需为每个本地工作区预计算派生标识做匹配集。
4. **Folder/Document 级独立密钥 + 继承**：v1 工作区级统一密钥，后续按需用 HKDF 派生（复杂度高）。

### 已解决（原开放问题）

- ~~离线时长限制自动撤销~~ → **不自动撤销**（撤销是显式 Owner 操作 + lazy 轮换）。
- ~~多设备同用户是否共享身份~~ → **每台设备独立身份**（设备级，与 SwarmDrop 一致）；"用户"仅 UI 归组。

## 分阶段实现

| 阶段 | 内容 | 密码学 |
|------|------|--------|
| **Phase 1** | 设备身份（Stronghold + 密钥对）、本地工作区/文件夹/文档 CRUD、权限表结构 | 密钥生成 |
| **Phase 2** | 设备配对（6位码）、已配对设备分享、Owner + Editor + Reader 同步 | read_key + write_key 分发 |
| **Phase 3** | 权限继承、链接分享（DHT 邀请、密码保护、有效期、使用次数） | invite_secret 加密、密钥包 |
| **Phase 4** | 权限撤销、密钥轮换、Owner 转让 | 密钥轮换、重新分发 |
