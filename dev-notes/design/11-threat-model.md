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
3. **"踢人即焚"（v1.1 已做两段式切断，但旧内容仍永久可读）**：移除成员现在会 **(a) 在线即时切断**——诚实成员对被移除 source 的 gossip 解密前用 `is_authorized` 预检直接丢弃(`coordinator::handle_ws_gossip_update`)，使其在线实时写入即刻失效；**(b) 密钥轮换**——`revoke_member` 发完 `Revoke` op 后调 `keys::rotate_workspace_key`(bump `key_version` + 新 self-Lockbox)，剩余成员收到 `PermissionOpsUpdate` 时反应式重取新版本(`coordinator` → `fetch_workspace_key`)，被移除设备拿不到新 Lockbox → 解不开**新内容**。但**撤销仍是 lazy 的**：被移除设备对其离开**前**已同步的旧内容(旧 `key_version`)**永久可读**、无法远程删除/收回(全行业物理边界)。在线切断不是密码学切断(对方仍持旧 key、能离线解已抓的旧密文)。离线成员在轮换后上线、拿到新 key 前用旧 key 解密合法(身份未变)。链接邀请的撤销同理靠轮换。
4. **恶意已授权设备**：v1「有 key 即可写」，逐 update 签名携带但不强制校验。拿到 write_key 的设备可注入伪造/损坏 update（CRDT 仍会合并）。真正的写权限强制（丢弃越权 update）= v2 Reader 时一并打开。
5. **元数据完全隐匿**：已配对设备能看到对方该工作区的**完整路径树**（rel_path/title 经 Noise RR 传，对端可见）。文件名加密是 v2+ 的元数据最小化。

## 已治理的元数据泄漏面

- **GossipSub topic 名**：现状 `swarmnote/ws/{明文 workspace_uuid}` 泄露"哪些 PeerId 关注哪个工作区"的协作兴趣图 + 工作区存在性 → 改 `swarmnote/ws/{HMAC(workspace_root_secret, "topic") 的 Base32}`；`CtrlMessage::WorkspaceOpened` 的 uuid 同样改派生标识。
  - 注意：topic 派生只**降低**元数据泄漏，**不替代** payload 加密（topic ≠ 访问凭证，Iroh 教训）。
- **DHT 邀请 key**：现状 `SHA256(ns||id)` 的 preimage 随记录上网可被存储节点看到、可枚举 → 改 HMAC 加盐不可逆派生，使存储节点无法反推语义/批量枚举。
- **awareness DoS**：即使加密，仍需出站本地节流（光标 debounce）+ 入站对每 PeerId 设频率上限。

## 权限链的信任根（authorization DAG genesis anchor）

权限链由 genesis op（自封 Owner，`prev_hash=None`）引导。**genesis 必须绑定到工作区的权威创建者**——`materialize` 只认 `issuer == target == workspaces.created_by` 的 genesis；任何其他设备自签的 genesis（签名/哈希自洽但 `issuer != created_by`）一律丢弃。这堵住了「伪造第二个 genesis 自封 Owner → 骗 key 持有者把 workspace key 封给攻击者」的越权面（一个仅配对、未授权的设备无法借此成为 Owner）。owner（`created_by`）还受保护永不被 revoke/降级（保证至少一个 Owner 存活）。

- **owner 设备**：`created_by` = 自己，创建时即正确。
- **joiner 设备**：本地行创建时 `created_by` = 自己；首次同步（`ensure_workspace_key`）收到经 Noise 认证的对端返回的链后，用链中唯一 genesis 的 issuer 把 `created_by` 钉成真 owner（**trust-on-first-use**）。

## 分享的知情同意（v1.1）

- 分享改为**邀请→接受双向握手**：owner 发 `ShareInvitation`、对端弹窗，**未接受不签发 grant op**（对端不被授权、拿不到 key）。这避免设备被静默塞进别人工作区（对齐 Matrix/MLS「处理才入组」）。拒绝/超时(90s)= 不授权。
- 用户可在设置里开启「自动接受协作邀请」(默认关)：开启后**对已配对设备的邀请自动接受**——这是用户主动选择把"已配对"提升为"信任邀请",等价于放弃这一层确认;仅对已配对设备生效(配对本身已是一次互信)。
- **未打开工作区的可发现性(headless 懒打开)**：成员工作区即使没在窗口打开,也会在被授权对端 `ListWorkspaces` 时按需 sync-only 打开并广播——**仍按 `role_of` 过滤**,未授权对端既看不到名字也拉不到内容,不放宽既有 gating。

## 残留风险（接受并记录）

- DHT 邀请**存在性**无法完全隐藏（存储节点知道"有这么一条记录"）；已用 HMAC key + 签名 + custom validator 缓解枚举与投毒。
- State vector 即使被 Noise 护住，仍泄露"有哪些 client、各做了多少操作"的协作图——因走点对点对账（不让盲中继看 SV 算 delta）、对端是已认证已配对设备，风险可接受。
- 并发成员变更可能把 key 泄露给本不该给的新人（p2panda 告警的去中心化竞态）；用 strong-removal 语义 + key_version 确定性收敛缓解。
- **joiner 的 owner-pin 是 TOFU**：joiner 首次同步时信任「它主动选择去同步的、经 Noise 认证的对端」所给链里的 genesis。若该对端恶意（M 给出只含 M 自签 genesis 的链），joiner 会把**它从 M 拉取的那个工作区副本**的 `created_by` 钉成 M。后果**仅限 joiner 自身本地副本**被污染（装入对方给的 key、无法解密真 owner 的内容）= 自伤式 DoS/错状态，**不构成 key 外泄**（M 没有真 workspace key；真 owner 的 `build_workspace_key_response` 用正确链拒绝非成员）、**不在诚实设备上升权**（真 owner 与其他成员的 `created_by` 仍拒伪造 genesis）。这是「从谁那同步就得到谁的工作区」的 P2P 固有性质。升级路径（v1.1+）：pin 前用邀请 token / 带外确认的成员身份作可信 anchor，或提供本地"重置 owner-pin"恢复手段。
- **入站 op 仅验签、不验来源成员身份**：`coordinator::handle_ctrl_message` 的 `PermissionOpsUpdate` 与 full_sync 收链只 `op.verify()` 即落库。已 pin 的设备安全（`materialize` 按 `created_by` 拒伪造 genesis）；但被邀请的恶意 Collaborator 可灌入大量自签合法 op 造成存储写放大 + 每次 `materialize` 重放的 CPU 开销（DoS，非 key 泄露）。升级路径：落库前校验来源 `role_of(source).is_some()` + 单消息 op 数上限。

## 升级路径（威胁模型若升级再做）

- 防恶意写入 / 真只读 → v2：逐 update 签名强制校验 + Reader 角色。
- 防丢设备 → 本地静态加密 / Megolm 式 ratchet。
- 真 PCS / 大群 → MLS（RFC 9420）/ BeeKEM（Keyhive）。
- 元数据最小化 → 文件名/路径加密、区间访问控制（Grappa）。
