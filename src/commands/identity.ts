import { invoke } from "@tauri-apps/api/core";

export interface DeviceInfo {
  peer_id: string;
  device_name: string;
  hostname: string;
  os: string;
  platform: string;
  arch: string;
  created_at: string;
}

export async function getDeviceInfo(): Promise<DeviceInfo> {
  return invoke<DeviceInfo>("get_device_info");
}

export async function setDeviceName(name: string): Promise<void> {
  return invoke("set_device_name", { name });
}
