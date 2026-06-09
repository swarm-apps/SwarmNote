import { useEffect, useRef, useState } from "react";

/**
 * 倒计时 hook：返回距 `expiresAt`（ISO 时间串）的剩余整秒数，每秒刷新；
 * 归零时调用 `onExpired` 一次。
 *
 * `onExpired` 用 ref 固化，所以即使调用方每次 render 传新的内联回调，interval
 * 也只在 `expiresAt` 变化时重建（而非每 render 重建）。
 */
export function useCountdown(expiresAt: string, onExpired: () => void): number {
  const [remaining, setRemaining] = useState(() =>
    Math.max(0, Math.ceil((new Date(expiresAt).getTime() - Date.now()) / 1000)),
  );

  const onExpiredRef = useRef(onExpired);
  onExpiredRef.current = onExpired;

  useEffect(() => {
    const interval = setInterval(() => {
      const left = Math.max(0, Math.ceil((new Date(expiresAt).getTime() - Date.now()) / 1000));
      setRemaining(left);
      if (left <= 0) {
        clearInterval(interval);
        onExpiredRef.current();
      }
    }, 1000);
    return () => clearInterval(interval);
  }, [expiresAt]);

  return remaining;
}
