# 威胁模型（E2E 分享 v1）

> **2026-06 定稿**。配套 [08-e2e-encryption.md](08-e2e-encryption.md) / [04-permissions.md](04-permissions.md) / [05-sharing.md](05-sharing.md)。
> 本文显式声明 SwarmNote v1 加密**防什么、不防什么**，避免给用户错误的安全预期。

## 加密主体与信任锚点

- 加密/授权主体是**设备**（per-device libp2p Ed25519，= PeerId = Noise 静态身份）。"用户"仅 UI 归组多 PeerId。
- 信任锚点：GossipSub 入站消息的 `from(PeerId)` 由 `StrictSign`（`MessageAuthenticity::Signed` + `ValidationMode::Strict`）已认证。**所有 UI 身份展示（尤其 awareness 的光标/用户名/在线）以 PeerId ↔ 已配对设备映射为准，绝不信任 payload 里自报字段**（Yjs awareness 不签名、可任意 spoof）。

## 防护边界（按对手）

| 对手 | 能看到 | 看不到 |
|------|--------|--------|
| **中继 / 引导节点（Relay/Bootstrap）** | 加密流量的存在、PeerId、连接元数据 | 任何文档明文（点对点 Noise + gossip payload 加密双重护住）|
| **GossipSub mesh 内转发节点（未授权）** | 加密 payload（密文）、消息大小/频率、派生后的 topic 标识 | 文档内容、光标/在线（payload 用 read_key 加密，无 key 解不开）|
| **网络窃听者** | TCP/QUIC 元数据 | 内容（Noise + payload 加密）|
| **DHT 存储节点** | online 宣告（公开存在性）、"存在一条邀请记录"、签名加密邀请包密文 | 邀请内的 workspace key（被 fragment secret 派生 key 加密，secret 不上 DHT）|
| **被移除的前协作者** | 它离开前已获取的明文/旧密文（**永久**，lazy 撤销）| 轮换之后用新 key 加密的新内容 |
| **持有完整链接（含 #secret）的人** | 该 key_version 下的工作区内容 | 轮换之后的新内容 |

## 明确**不防**的（写清楚，别承诺）

1. **丢设备 / 本地磁盘取证**：授权设备本地写明文 `.md`（folder-is-truth 的代价），交给 OS 全盘加密。v1 不做本地静态加密、不上 StrongBox/锁屏门控。
2. **前向保密（FS）/ 后泄露安全（PCS）**：CRDT 必须重放全历史 → 必须保留全部历史 key，应用层 FS 名存实亡（Keyhive 已论证）。不做 ratchet。
3. **"踢人即焚"**：撤销是 lazy 的——被移除设备/已存链接者对其能访问过的旧内容**永久可读**。真正切断 = 轮换 key 让新内容对其失效。
4. **恶意已授权设备**：v1「有 key 即可写」，逐 update 签名携带但不强制校验。拿到 write_key 的设备可注入伪造/损坏 update（CRDT 仍会合并）。真正的写权限强制（丢弃越权 update）= v2 Reader 时一并打开。
5. **元数据完全隐匿**：已配对设备能看到对方该工作区的**完整路径树**（rel_path/title 经 Noise RR 传，对端可见）。文件名加密是 v2+ 的元数据最小化。

## 已治理的元数据泄漏面

- **GossipSub topic 名**：现状 `swarmnote/ws/{明文 workspace_uuid}` 泄露"哪些 PeerId 关注哪个工作区"的协作兴趣图 + 工作区存在性 → 改 `swarmnote/ws/{HMAC(workspace_root_secret, "topic") 的 Base32}`；`CtrlMessage::WorkspaceOpened` 的 uuid 同样改派生标识。
  - 注意：topic 派生只**降低**元数据泄漏，**不替代** payload 加密（topic ≠ 访问凭证，Iroh 教训）。
- **DHT 邀请 key**：现状 `SHA256(ns||id)` 的 preimage 随记录上网可被存储节点看到、可枚举 → 改 HMAC 加盐不可逆派生，使存储节点无法反推语义/批量枚举。
- **awareness DoS**：即使加密，仍需出站本地节流（光标 debounce）+ 入站对每 PeerId 设频率上限。

## 残留风险（接受并记录）

- DHT 邀请**存在性**无法完全隐藏（存储节点知道"有这么一条记录"）；已用 HMAC key + 签名 + custom validator 缓解枚举与投毒。
- State vector 即使被 Noise 护住，仍泄露"有哪些 client、各做了多少操作"的协作图——因走点对点对账（不让盲中继看 SV 算 delta）、对端是已认证已配对设备，风险可接受。
- 并发成员变更可能把 key 泄露给本不该给的新人（p2panda 告警的去中心化竞态）；用 strong-removal 语义 + key_version 确定性收敛缓解。

## 升级路径（威胁模型若升级再做）

- 防恶意写入 / 真只读 → v2：逐 update 签名强制校验 + Reader 角色。
- 防丢设备 → 本地静态加密 / Megolm 式 ratchet。
- 真 PCS / 大群 → MLS（RFC 9420）/ BeeKEM（Keyhive）。
- 元数据最小化 → 文件名/路径加密、区间访问控制（Grappa）。
