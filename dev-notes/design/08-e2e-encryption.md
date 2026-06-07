# E2E 加密设计（v1 定稿）

> **2026-06 调研定稿**，取代 2026-03 基于 SecSync 的初稿。主要变化：加密主体由"用户"改为**设备**、密钥存储由 Stronghold 改为 **OS keychain**、X25519 由设备 Ed25519 **复用派生**、新增 **key commitment**。
> 本文描述加密底层；权限/角色见 [04-permissions.md](04-permissions.md)，分享流程见 [05-sharing.md](05-sharing.md)，威胁边界见 [11-threat-model.md](11-threat-model.md)。
>
> 业界共识：SwarmNote 这种「无服务器 + Yjs CRDT + 2-3 人 + 偶尔分享」的场景，正解是 **per-workspace 对称群 key + per-device X25519 Lockbox + 移除时 lazy 轮换**——与 Jazz/cojson、p2panda Data Encryption、Tresorit、SecSync 同构。MLS/BeeKEM/CGKA 对此规模是过度工程，后置 v2/v3。

## 设计原则

- **仅传输加密**：加密只发生在网络层（GossipSub 广播 / 资产传输 / 链接邀请包）。授权设备本地仍写明文 `.md`，folder-is-truth 不变；丢设备的风险交给 OS 全盘加密。
- **加密主体 = 设备**：SwarmNote 无账号、无用户注册表，唯一稳定密码学身份是 per-device 的 libp2p Ed25519 keypair（`crates/core/src/identity.rs`，已作 PeerId + Noise 静态身份持久化在 OS keychain）。Lockbox 收方、`permissions.peer_id` 全是设备粒度；逻辑上的"用户"只是 UI 把若干 PeerId 归组，密码学上不存在用户主体。
- **不追前向保密（FS）**：CRDT 必须重放全部历史 update 才能收敛 → 必须保留所有历史 key，应用层 FS 名存实亡（Ink & Switch Keyhive 已论证）。保留 `{key_version → key}` 全历史是正确取舍。

## 密钥层级

```text
第0层 设备根（OS keychain）
  Ed25519 设备密钥 ──复用派生──▶ X25519 设备密钥（Lockbox 收方）

第1层 workspace 对称密钥（每 workspace 一组，带 key_version）
  ├── read_key  (32B 随机)  → 加密 gossip doc-update / awareness / 资产
  └── write_key (32B 随机，独立生成，绝不从 read_key 派生)
                            → v1 仅作 Collaborator 写凭证占位；v2 演化为逐 update 签名授权根
  （admin_key_enc 列保留，v1 不用）

第2层 子用途派生（每次加密前 HKDF-Expand）
  HKDF-SHA256(read_key, info = b"swarmnote:v1:<purpose>" || workspace_id || key_version)
  purpose ∈ { gossip, asset, commit }    salt = workspace_id
```

**为什么 v1 就分两把独立 key**：v1 虽不强制只读，但若 read/write 合成一把或派生，v2 给 Reader 发 read_key 就等于泄露能推 write 凭证的种子，被迫 re-key 重构。两把独立 ⇒ v2 加 Reader = 「不发 write_key 的 lockbox + 打开签名校验开关」，schema/密钥布局零改动。这是把「2 级 → 3 级」从重构降为开关的关键。

**key 历史**：本地保存 `{key_version → (read_key, write_key)}` 全历史 map。解旧密文按 payload 头部携带的 `key_version` 取对应 key。不做 Plutus 式单向 key 链（2-3 人是过度优化）。

## Ed25519 → X25519 派生

采用 **复用 + 严格 HKDF 域分离**，而非每设备独立生成第二把长期 X25519：

- 本机 X25519 私钥 = `SigningKey::to_scalar_bytes()`（ed25519-dalek 2.x）；对端 X25519 公钥 = `verifying_key().to_montgomery()`（或等价 libsodium `crypto_sign_ed25519_*_to_curve25519`）。
- **理由**：配对时对端只交换 PeerId/Ed25519 公钥即可算出其 X25519 公钥，零额外密钥分发/签名绑定；Thormarker 2021/509 在 ROM 下证明 joint security，libsodium/GNUnet 生产在用。
- **硬约束**：① 全局钉死同一 clamp 约定（`to_scalar_bytes` 配 `to_montgomery`，跨端必须一致否则 DH 不匹配）；② **只做单层转换，不做层级派生**（层级派生需乘 cofactor，否则触发 hidden-number-problem）；③ Lockbox 对称封装必须 key-committing（见下）。

## 对称加密原语

- **XChaCha20-Poly1305**：192-bit nonce，每条消息 `OsRng` 随机生成 nonce 前置于密文——免去跨设备 nonce 计数器协调（P2P 多写的唯一现实选择；约 2^80 条消息才到 2^-32 碰撞概率）。这正是 SecSync/p2panda Data Encryption 都选它的原因。
- **key commitment**（纸面遗漏，必补）：裸 AEAD 不是 key-committing——同一密文可被构造成不同 key 解出不同明文，链接分享是这类 partitioning-oracle / invisible-salamander 攻击（USENIX'21）的靶心。方案：HKDF 多挤 32B（`info = b"swarmnote:v1:commit"...`）作 commitment 与密文同存，解密时用 `subtle` 常量时间比对。零新依赖。

## 逐通道加密策略

| 通道 | 是否应用层加密 | 说明 |
|------|:---:|------|
| **gossip doc-update**（`ws` topic） | ✅ 必须 | 真正裸奔点：mesh 内任何订阅 topic 的转发节点解开逐跳 Noise 后读到明文。现状 `[16B uuid][明文 update]` 零保护 |
| **awareness**（`ws-aw` topic） | ✅ 必须 | 不加密会泄露光标/在线/用户名/颜色给整个 mesh。AAD 的 `msg_type` 区分 ws/ws-aw 防跨通道重放 |
| **资产分块**（走 gossip 时） | ✅ 必须 | 各分块独立 nonce + AAD（含 `asset_id+chunk_index`）防重排/跨资产重放 |
| **SV 交换 / 全量拉取 / 资产 RPC** | ❌ 不叠 | 走 request-response，已被 libp2p Noise 端到端加密 + 对端认证（点对点直连不经 mesh 转发）。应用层加密预算全砸 GossipSub |
| **DocList 元数据（路径/标题）** | ⚠️ 真正泄漏在 topic 名 | payload 走 RR 由 Noise 护住；但 `swarmnote/ws/{明文uuid}` topic 名泄露"谁关注哪个工作区"→ 改 HMAC 不可逆派生 topic（见 [11-threat-model.md](11-threat-model.md)） |
| **online 宣告（DHT）** | 现状即可 | 本就是公开存在性信息；但**分享邀请包**发 DHT 必须加密+签名（见 [05-sharing.md](05-sharing.md)） |

## 加密 wire 格式（GossipSub payload）

```text
[1B version][4B key_version][24B nonce][32B commitment][ciphertext]
AAD = workspace_id(16B) || doc_uuid(16B) || key_version(4B) || msg_type(1B)
```

- `doc_uuid` 仍需明文（路由用），放进 AAD 绑定（防混淆）。
- 改造点：在 `ydoc.on('update')` 拿到 update 后**先 `mergeUpdates` 合并、再加密广播**（别逐 keypress 加密——会让文档膨胀且无法压缩，Keyhive/Automerge 反面教训）。
- 解密失败 / `key_version` 不符 / 反序列化失败 → GossipSub v1.1 **Extended Validator 返回 `Reject`**（触发 P4 评分惩罚 + graylist），顺带补上"入站 gossip 无来源鉴权"的审计缺口。

## Rust 依赖（钉死稳定线，不上 rc）

| 用途 | 选择 | crate |
|------|------|-------|
| 对称 AEAD | XChaCha20-Poly1305 | `chacha20poly1305 = "0.10"` |
| 非对称 DH | X25519 ECDH | `x25519-dalek = "2"` |
| 设备签名 + Ed25519→X25519 | Ed25519 | `ed25519-dalek = "2"`（复用 libp2p 同一把设备密钥）|
| 密钥派生 + key commitment | HKDF-SHA256 | `hkdf = "0.13"` + `sha2 = "0.10"` |
| 链接密码 KDF | Argon2id（RFC 9106 参数二 t=3/m=64MiB/p=4）| `argon2 = "0.5"` |
| 常量时间比较 | — | `subtle = "2"` |
| CSPRNG | OS 熵 | `getrandom`（经 `rand`）|

## 与 2026-03 纸面设计的分歧

1. **主体设备非用户**：无账号系统就没有"用户"这个密码学锚点，强造用户主体反而要引入账号/同步用户密钥的复杂度。
2. **OS keychain 非 Stronghold**：主体是设备 + 不防丢设备，Stronghold 的"用户主密钥"前提不成立且强度过度。
3. **Ed25519 复用派生 X25519**：纸面未明确 X25519 来源；定为单层复用派生（钉死 clamp）。
4. **新增 key commitment**：纸面漏了，链接分享非补不可。
5. **撤销是 lazy 的**，不是即时彻底（见 [04-permissions.md](04-permissions.md)）。
