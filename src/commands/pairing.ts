import { invoke } from "@tauri-apps/api/core";

export interface PairingCodeInfo {
  code: string;
  createdAt: string;
  expiresAt: string;
}

export interface PairedDeviceInfo {
  peerId: string;
  hostname: string;
  os: string;
  platform: string;
  arch: string;
  pairedAt: string;
  lastSeen: string | null;
  isOnline?: boolean;
  rttMs?: number;
  connection?: ConnectionType;
}

export interface PeerInfo {
  peer_id: string;
  hostname: string;
  os: string;
  platform: string;
  arch: string;
  is_connected: boolean;
  rtt_ms: number | null;
  connection_type: string | null;
}

// ── 统一设备类型（对齐后端 Device struct） ──

export type DeviceStatus = "online" | "offline";
export type ConnectionType = "lan" | "dcutr" | "relay";

export interface Device {
  peerId: string;
  name?: string;
  hostname: string;
  os: string;
  platform: string;
  arch: string;
  status: DeviceStatus;
  connection?: ConnectionType;
  latency?: number;
  isPaired: boolean;
  pairedAt?: string;
  lastSeen?: string;
}

export interface DeviceListResult {
  devices: Device[];
  total: number;
}

export type DeviceFilter = "all" | "connected" | "paired";

// ── 工作区列表交换 ──

export interface RemoteWorkspaceInfo {
  uuid: string;
  name: string;
  docCount: number;
  updatedAt: number;
  peerId: string;
  peerName: string;
  isLocal: boolean;
}

export async function getRemoteWorkspaces(): Promise<RemoteWorkspaceInfo[]> {
  return invoke("get_remote_workspaces");
}

export async function listDevices(filter?: DeviceFilter): Promise<DeviceListResult> {
  return invoke("list_devices", { filter });
}

export interface ShareCodeDeviceInfo {
  peerId: string;
  osInfo: {
    name?: string;
    hostname: string;
    os: string;
    platform: string;
    arch: string;
  };
}

export type PairingMethod = { type: "Direct" } | { type: "Code"; code: string };

export interface PairingResponse {
  status: "Success" | "Refused";
  reason?: string;
}

export async function generatePairingCode(expiresInSecs?: number): Promise<PairingCodeInfo> {
  return invoke("generate_pairing_code", { expiresInSecs });
}

export async function getDeviceByCode(code: string): Promise<ShareCodeDeviceInfo> {
  return invoke("get_device_by_code", { code });
}

export async function requestPairing(
  peerId: string,
  method: PairingMethod,
  remoteOsInfo?: { name?: string; hostname: string; os: string; platform: string; arch: string },
): Promise<PairingResponse> {
  return invoke("request_pairing", { peerId, method, remoteOsInfo });
}

export async function respondPairingRequest(pendingId: number, accept: boolean): Promise<void> {
  return invoke("respond_pairing_request", { pendingId, accept });
}

export async function getPairedDevices(): Promise<PairedDeviceInfo[]> {
  return invoke("get_paired_devices");
}

export async function unpairDevice(peerId: string): Promise<void> {
  return invoke("unpair_device", { peerId });
}

export async function getNearbyDevices(): Promise<Device[]> {
  return invoke("get_nearby_devices");
}
