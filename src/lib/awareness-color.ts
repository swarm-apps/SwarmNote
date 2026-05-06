// Awareness color palette + deterministic device-id mapping.
//
// Same logic must exist in mobile (src/lib/awareness-color.ts) so two
// devices computing the color independently from a peer_id agree.

export const AWARENESS_PALETTE = [
  "#FFA94D", // honey orange
  "#7AB548", // honeycomb green
  "#4A90E2", // sky blue
  "#E94B7C", // coral pink
  "#9B59B6", // purple
  "#F4D03F", // honey yellow
  "#1ABC9C", // teal
  "#E67E22", // amber
] as const;

export function colorForDevice(deviceId: string): string {
  let h = 0;
  for (let i = 0; i < deviceId.length; i++) {
    h = ((h << 5) - h + deviceId.charCodeAt(i)) | 0;
  }
  return AWARENESS_PALETTE[Math.abs(h) % AWARENESS_PALETTE.length];
}
