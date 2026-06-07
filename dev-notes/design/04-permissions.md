# 权限模型（v1 定稿）

> **2026-06 调研定稿**，取代 2026-03 的三级（Owner/Editor/Reader）纸面版。v1 收敛为**两级 Owner/Collaborator**（真正的只读 Reader 后置 v2），并把权限变更做成**签名操作链**而非可变 DB 行。
> 加密底层见 [08-e2e-encryption.md](08-e2e-encryption.md)，分享流程见 [05-sharing.md](05-sharing.md)。

## 角色（v1 两级）

| 角色 | 读 | 写 | 管理成员 | 密钥持有 |
|------|:--:|:--:|:--------:|---------|
| **Owner** | ✓ | ✓ | ✓（授权/移除/轮换）| read_key + write_key（+ 管理权）|
| **Collaborator** | ✓ | ✓ | | read_key + write_key |

- **v1 不做真正的只读 Reader**：有 key 即可读写。原因——真 Reader 必须每条 update 设备签名 + 合并前校验 + 丢弃越权写入，是巨大工作量。
- **v2 Reader 是开关不是重构**：v1 已（a）read/write 分两把独立 key、（b）每条 update 携带设备 Ed25519 签名（携带但不强制校验）。v2 加 Reader = 「只发 read_key 的 lockbox + 打开签名校验丢弃越权 update」，无新表、无 re-key、无历史回填。

## 密码学执行（P2P 无服务器）

没有服务器强制权限，用**密钥分发 + 签名**替代：

- **读 = 持有 read_key**：能解密 = 能读。通过 X25519 Lockbox 分发（见 [05-sharing.md](05-sharing.md)）。
- **写 = 持有 write_key**：v1「有 key 即可写」，update 携带设备签名但**不强制校验**（为 v2 铺路）。
- **管理 = Owner**：成员变更操作必须由 Owner 的设备 Ed25519 签名（见下）。

## 权限表 = 签名操作链（v1 就做）

**不能**把成员/角色当普通可变 DB 行在 P2P 间同步——任何节点都能伪造一行把自己设成 Owner。做成 **append-only 的签名操作 DAG**（对齐 Matrix auth chain / Jazz 角色 transaction / p2panda Causal-Length CRDT）：

```rust
// 新增 permission_ops 表（append-only）
struct PermissionOp {
    op: OpKind,              // Add / Remove / Promote / TransferOwner
    target_peer_id: String,  // 被操作设备
    new_role: Role,
    issuer_peer_id: String,  // 发起设备
    key_version: i32,        // 关联的 workspace key 版本
    prev_hash: Vec<u8>,      // 因果前序（构成 DAG/链）
    issuer_signature: Vec<u8>, // issuer 设备 Ed25519 签名（覆盖以上全部字段）
}
```

每个节点收到后**本地重放并校验**三条不变量：

1. **签名有效**：`issuer_signature` 由 `issuer_peer_id` 对应的 Ed25519 公钥验证通过。
2. **issuer 有权**：在该操作的因果前序里，`issuer` 确实是 Owner（或对该操作有权）。
3. **不得提权越级**：不能把任何人设到高于 issuer 自己的等级（Matrix 防提权不变量）。

任一不满足 → 丢弃该 op。改一条记录就得伪造整条签名链（不可行）。这使无中心、离线可判定。

> **当前 schema 缺口**：现有 `permissions` 表是普通行结构（`id/resource_type/resource_id/peer_id/role/granted_by/granted_at`）。v1 需新增 `permission_ops` 表承载签名操作链；`permissions` 可作为重放后的物化视图（可重建索引）。

## 撤销与密钥轮换（lazy re-encryption）

移除某 Collaborator 设备时：

1. Owner 生成新 `read_key` + `write_key`，`key_version + 1`。
2. **只**为剩余设备重新封 Lockbox（新增 `workspace_key_lockboxes` 行），不给被移除设备。
3. 此后新 gossip update / awareness / 资产用新 key 加密，payload 头部 `key_version` 标新版本。
4. **旧密文不重写**——去中心化下旧 update 已散落各设备、各人本地有旧 key，重写既不干净又破坏 CRDT 历史连续性（SecSync 能丢旧 snapshot 是因为有中心 relay，SwarmNote 没有）。
5. permissions 记一条签名的 `Remove` op（非裸删行）。

**key 历史**永久保留 `{key_version → (read_key, write_key)}`（CRDT 必须能重放全历史）。

### 并发撤销收敛

去中心化下两设备可能并发触发轮换，致 `key_version` 冲突。用确定性收敛——`key_version` 用 `(counter, 触发者 PeerId)` 做全序 tie-break，或并发时两个新 key 都保留（谁都能解）直到下次轮换合并（借鉴 BeeKEM coordination-free revocation 思想，不上 BeeKEM 本体）。

### 诚实声明（写进威胁模型）

撤销是 **lazy** 的：被移除设备保留它离开前的 read_key，**对它离开前能看到的明文 `.md` / 旧密文永久可读**。撤销只保证「读不到轮换之后的新内容」。这是所有本地优先 E2EE 方案的固有属性（Jazz/SecSync/Keyhive 均如此），与「不防丢设备」决策自洽。**别向用户承诺「踢人即焚」。** 详见 [11-threat-model.md](11-threat-model.md)。

## 权限粒度

- **v1 = 工作区级统一密钥**：一个 workspace 一组 read/write key，覆盖其下所有文档。
- **Folder/Document 级独立密钥 + 继承（向下级联 + 可覆盖 + 最高权限胜出）**：复杂度高，后置 v2+。届时用 HKDF 从 workspace key 派生 folder/doc key。
