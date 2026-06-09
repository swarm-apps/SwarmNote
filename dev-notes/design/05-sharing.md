# 分享机制（v1 定稿）

> **2026-06 调研定稿**。两条路径：① 已配对设备/人之间用 **X25519 Lockbox** 直传密钥；② 给陌生人用 **Mega 式链接分享**（URL fragment 内嵌密钥 + DHT 签名邀请包）。
> 加密底层见 [08-e2e-encryption.md](08-e2e-encryption.md)，权限见 [04-permissions.md](04-permissions.md)，边界见 [11-threat-model.md](11-threat-model.md)。

## v1.1 实现增量（2026-06，分支 feature/sharing-v1.1，OpenSpec `sharing-v1-1`）

v0.5.0 落地了路径一的密钥分发与权限内核;v1.1 把分享生命周期补全(均已编译 + core 106 tests 绿):

- **邀请→接受握手**：路径一不再是「owner 单方推 grant」,而是 owner 发 `WorkspaceRequest::ShareInvitation`(`AppCore::invite_device`,阻塞等响应)→ 对端经通知队列弹 `ShareInvitationDialog` → **接受后 owner 才 `grant_collaborator`**(未接受不授权)。复用配对的 request-response pending 关联机制。可在设置「网络」tab 开启「自动接受协作邀请」(默认关)对已配对设备免确认。
- **撤销 = 在线切断 + 密钥轮换**(兑现本文档原「撤销链接 = 轮换 workspace key」的承诺,即 e2e-sharing-v1 延后的 5.4)：`revoke_member` 发完 `Revoke` op 后调 `keys::rotate_workspace_key`(bump `key_version` + 新 self-Lockbox);剩余成员收到 `PermissionOpsUpdate` 时反应式重取新 key(`coordinator` → `fetch_workspace_key`),被移除设备解不开新内容;同时 gossip 入站按 `is_authorized(source)` 预检即时丢弃被移除设备的广播。lazy:旧内容仍可读(见威胁模型)。被移除设备收 `MemberRevoked` 事件自动退订 + 提示。
- **未打开工作区可同步(headless 懒打开)**：`AppCore::ensure_open_for_sync` 在被授权对端 `ListWorkspaces`/`DocList`/`WorkspaceKey` 时按需以 sync-only(不订阅 gossip)打开本机未在窗口打开的工作区,使其可被发现/拉取(仍 `role_of` 过滤)。
- **未做(延后)**：接受邀请后 auto-join(需定落盘位置)、被分享方持久「分享给我」清单、链接分享(路径二)、双机实测、i18n。

## 路径一：配对设备/人分享（Lockbox）

前提：双方已完成现有配对流程（`PairingManager` 已交换 PeerId/Ed25519 公钥并建立信任）。

```text
A(Owner) 选 workspace + 选已配对设备 B + 选角色（Owner/Collaborator）
  1. A 由 B 的 Ed25519 公钥算出 B 的 X25519 公钥（to_montgomery）
  2. A 用 X25519(A_sk, B_pk) ECDH → HKDF 派生封装 KEK
  3. 用 KEK + XChaCha20-Poly1305 把当前 read_key+write_key(+key_version) 封成一个 Lockbox（带 key commitment）
  4. Lockbox 经 request-response（Noise 已加密）发给 B，或随 workspace_keys 同步落 B 的 workspace.db
  5. B 用自己的 X25519 私钥解开 Lockbox → 写入本地 {key_version→key} map → 加入同步
  6. A 记一条签名的 Add(Collaborator) permission_op
```

- 新增协作设备 = 多生成一个 Lockbox 行；不重写历史。复杂度 O(剩余设备数)，2-3 人无所谓。
- 这就是 Jazz 的 read key reveal / Tresorit 的非对称 share key 分发 / p2panda HPKE-X25519 的成品形态。

### Schema gap（必补）

当前 `workspace_keys.read_key_enc` 是**单 blob**，表达不了"每设备一份 Lockbox"。新增：

```rust
// workspace.db 新表
struct WorkspaceKeyLockbox {
    workspace_id: Uuid,
    key_version: i32,
    recipient_peer_id: String,   // 收方设备
    sealed_read_key: Vec<u8>,    // 用收方 X25519 公钥封装
    sealed_write_key: Vec<u8>,   // v2 Reader 时此字段留空
    commitment: Vec<u8>,
    sealed_by_peer_id: String,   // 封装方
}
```

## 路径二：链接分享给陌生人（Mega 式）

`swarmnote://invite/<token>#<secret>` —— `token` 是 DHT 寻址句柄（可走任何信道），`#secret`（fragment）是真正的解密密钥，**永不上 DHT/日志/剪贴板**。

### 生成（Owner 设备）

```text
1. OsRng 生成高熵 secret(≥256-bit) 与 token(DHT 寻址句柄)
2. 从 secret HKDF 派生：① 内层 DEK(封 workspace key) ② DHT 寻址 HMAC 材料 ③ commitment
   铁律：token 与 #secret 不同源；DHT key 与 #secret 解耦
3. 邀请包 payload = {
     role(owner/collaborator), resource_type, resource_id, key_version,
     sealed_workspace_keys（DEK + XChaCha20-Poly1305 封装 read_key+write_key）,
     commitment, expires_at（签进包内）
   }
4. 若设密码：再用 Argon2id(password, salt) 派生 KEK 把内层 DEK 再包一层
   （正交叠加 = 拿到链接 + 知道密码缺一不可）
5. 整包用 Owner 设备 Ed25519 签名
6. 发布到 DHT：key = HMAC(secret 派生材料, 固定 label)（不可逆/不可枚举，非明文 token/非 workspace_id）
   value = 签名的不透明密文邀请包；注册 custom validator（默认 validator 只收 pk/ipns），retrieval 验签防投毒
7. share_invites 落库（token/resource/role/encrypted_keys/expires_at/password_hash + 新增 commitment 列）
```

### 兑换（陌生人设备）

```text
1. deep link 处理器把整条 URL 交给 Rust 后端解析，解析后立即清掉 fragment
2. token → 派生 DHT key → DHT get → 验 Owner 签名 → 本地校验 expires_at（过期拒）
3. 若有密码：提示输入（强提示用户密码走另一条信道，与链接分开传）→ Argon2id 解外层
4. #secret 派生 DEK → 常量时间比对 commitment → 解 sealed_workspace_keys
   → 得 workspace key + key_version → 写本地 map → 加入工作区同步
```

### 安全约束与诚实声明

- `#secret` 永不进 DHT value / tracing 日志（整条 URL 当机密脱敏）/ 剪贴板历史 / 配置 store。
- **不做 max_uses 强制**：无中心下计数不可靠，会给用户「限了次数」的安全错觉。保留 schema 列但不承诺；真正撤销 = key rotation。
- **有效期是软过期**（DHT TTL + republish，恶意/缓存节点可保留），故 `expires_at` 签进包内本地强制 + 设短一点。
- **撤销链接 = 轮换 workspace key**：已保存完整链接（含 fragment 密钥）的人对那个 key_version 下的内容永久可读，删 DHT 记录不可靠（会被 republish/缓存）。Mega 自己也只能建议"换链接/移到新文件夹"。
- `token#secret` 当不可分割的不透明整体，UI 禁止用户手动截断（Tahoe-LAFS 编辑式降权血泪）。
- **Argon2id 参数**：RFC 9106 参数二 t=3 / m=64MiB / p=4 / 128-bit salt / 256-bit tag（移动端友好；桌面可上参数一）。

## 两种分享对比

| | 配对分享 | 链接分享 |
|--|---------|---------|
| 前提 | 已配对 | 不需要 |
| 密钥传输 | X25519 Lockbox 经 Noise 通道 | fragment 内嵌 + DHT 存签名加密包 |
| 双方离线异步兑换 | — | ✓（DHT 发布） |
| 密码保护 | 不需要 | 可选（Argon2id） |
| 有效期 | 持久授权 | 软过期 |
| 撤销 | 移除 + 轮换 | 轮换（删链接不可靠） |
| 场景 | 自己的多设备 / 信任的人 | 发给同事/朋友 |
