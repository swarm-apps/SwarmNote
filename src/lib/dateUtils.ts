import { i18n } from "@lingui/core";

export function formatRelativeTime(input: string | null): string {
  if (input == null) return i18n._("从未在线");

  const timestamp = new Date(input).getTime();
  const diff = Date.now() - timestamp;
  const minutes = Math.floor(diff / 60_000);

  if (minutes < 1) return i18n._("刚刚");
  if (minutes < 60) return i18n._("{minutes} 分钟前", { minutes });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return i18n._("{hours} 小时前", { hours });
  const days = Math.floor(hours / 24);
  if (days < 30) return i18n._("{days} 天前", { days });

  return new Date(timestamp).toLocaleDateString();
}

export function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  });
}
