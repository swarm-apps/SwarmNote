/**
 * Releases 数据管线 —— 构建时拉取 GitHub Releases，归一为统一 schema。
 *
 * 调用入口：`await loadReleases()`，在 Astro frontmatter 内使用。
 * 行为：
 *   1. 优先从 GitHub REST API 拉取主仓 + 移动端仓库的 release 列表
 *   2. 任何失败（网络/4xx/5xx/超时） → fallback 到 git-tracked cache
 *   3. 成功时刷新 cache（仅 build 模式下写盘；dev 不写）
 *   4. 通过 GITHUB_TOKEN 环境变量提升速率限制（CI 必备）
 */

import { promises as fs } from "node:fs";
import path from "node:path";

// ---------- 类型 ----------
export type Platform = "windows" | "macos" | "linux" | "android" | "other";
export type Arch = "x64" | "arm64" | "universal" | "unknown";

export interface ReleaseAsset {
  name: string;
  downloadUrl: string;
  size: number;
  contentType: string;
  platform: Platform;
  arch: Arch;
}

export interface NormalizedRelease {
  version: string; // 例 "0.3.1"（去前导 v）
  tagName: string; // 例 "v0.3.1"
  name: string;
  publishedAt: string; // ISO 8601
  body: string;
  url: string;
  assets: ReleaseAsset[];
}

export interface ReleasesData {
  desktop: {
    latest: NormalizedRelease | null;
    history: NormalizedRelease[];
  };
  mobile: {
    latest: NormalizedRelease | null;
  };
  fetchedAt: string;
  source: "live" | "cache" | "empty";
}

// ---------- 配置 ----------
const DESKTOP_REPO = "yexiyue/SwarmNote";
const MOBILE_REPO = "yexiyue/SwarmNote-RN";
const HISTORY_LIMIT = 10;
const FETCH_TIMEOUT_MS = 10_000;

// cache 文件位置（git-tracked，相对 docs 工作目录解析；Astro build 时 cwd = docs/）
const CACHE_PATH = path.resolve(process.cwd(), "src/data/releases.cache.json");

// ---------- 资产 → 平台/架构 分类 ----------
export function classifyAsset(filename: string): { platform: Platform; arch: Arch } {
  const f = filename.toLowerCase();

  // Windows
  if (f.endsWith(".msi")) return { platform: "windows", arch: "x64" };
  if (f.includes("_x64-setup.exe") || f.endsWith("-setup.exe") || f.endsWith(".exe")) {
    return { platform: "windows", arch: "x64" };
  }

  // macOS
  if (f.endsWith(".dmg")) {
    if (f.includes("aarch64") || f.includes("arm64") || f.includes("apple-silicon")) {
      return { platform: "macos", arch: "arm64" };
    }
    if (f.includes("x64") || f.includes("x86_64") || f.includes("intel")) {
      return { platform: "macos", arch: "x64" };
    }
    return { platform: "macos", arch: "universal" };
  }

  // Linux
  if (f.endsWith(".appimage") || f.endsWith(".deb") || f.endsWith(".rpm")) {
    if (f.includes("aarch64") || f.includes("arm64")) {
      return { platform: "linux", arch: "arm64" };
    }
    return { platform: "linux", arch: "x64" };
  }

  // Android
  if (f.endsWith(".apk") || f.endsWith(".aab")) {
    return { platform: "android", arch: "universal" };
  }

  return { platform: "other", arch: "unknown" };
}

// ---------- API 抓取 ----------
type GhAsset = {
  name: string;
  browser_download_url: string;
  size: number;
  content_type: string;
};
type GhRelease = {
  tag_name: string;
  name: string | null;
  published_at: string;
  body: string | null;
  html_url: string;
  draft: boolean;
  prerelease: boolean;
  assets: GhAsset[];
};

function authHeaders(): Record<string, string> {
  const headers: Record<string, string> = {
    Accept: "application/vnd.github+json",
    "X-GitHub-Api-Version": "2022-11-28",
    "User-Agent": "swarmnote-website-build",
  };
  const token = process.env.GITHUB_TOKEN || process.env.GH_TOKEN;
  if (token) headers.Authorization = `Bearer ${token}`;
  return headers;
}

async function ghFetch<T>(url: string): Promise<T> {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), FETCH_TIMEOUT_MS);
  try {
    const res = await fetch(url, {
      headers: authHeaders(),
      signal: ctrl.signal,
    });
    if (!res.ok) {
      throw new Error(`GitHub API ${res.status} ${res.statusText} on ${url}`);
    }
    return (await res.json()) as T;
  } finally {
    clearTimeout(timer);
  }
}

function normalizeRelease(r: GhRelease): NormalizedRelease {
  const tag = r.tag_name;
  const version = tag.replace(/^v/, "");
  return {
    version,
    tagName: tag,
    name: r.name?.trim() || tag,
    publishedAt: r.published_at,
    body: r.body ?? "",
    url: r.html_url,
    assets: r.assets.map((a) => ({
      name: a.name,
      downloadUrl: a.browser_download_url,
      size: a.size,
      contentType: a.content_type,
      ...classifyAsset(a.name),
    })),
  };
}

async function fetchRepoReleases(
  repo: string,
): Promise<{ latest: NormalizedRelease | null; history: NormalizedRelease[] }> {
  const list = await ghFetch<GhRelease[]>(
    `https://api.github.com/repos/${repo}/releases?per_page=${HISTORY_LIMIT}`,
  );
  const visible = list.filter((r) => !r.draft);
  const stable = visible.filter((r) => !r.prerelease);
  const latest = (stable[0] ?? visible[0]) ?? null;
  return {
    latest: latest ? normalizeRelease(latest) : null,
    history: visible.map(normalizeRelease),
  };
}

// ---------- Cache I/O ----------
async function readCache(): Promise<ReleasesData | null> {
  try {
    const raw = await fs.readFile(CACHE_PATH, "utf-8");
    return JSON.parse(raw) as ReleasesData;
  } catch {
    return null;
  }
}

async function writeCache(data: ReleasesData): Promise<void> {
  try {
    await fs.mkdir(path.dirname(CACHE_PATH), { recursive: true });
    await fs.writeFile(CACHE_PATH, `${JSON.stringify(data, null, 2)}\n`, "utf-8");
  } catch (err) {
    console.warn(`[releases] cache write failed: ${(err as Error).message}`);
  }
}

// ---------- 模块级 memo（同一次构建只跑一次） ----------
let memo: Promise<ReleasesData> | null = null;

export function loadReleases(): Promise<ReleasesData> {
  if (memo) return memo;
  memo = (async () => {
    const fetchedAt = new Date().toISOString();
    let desktop: ReleasesData["desktop"] | null = null;
    let mobile: ReleasesData["mobile"] | null = null;
    let liveOk = false;

    // 桌面端必拉
    try {
      desktop = await fetchRepoReleases(DESKTOP_REPO);
      liveOk = true;
    } catch (err) {
      console.warn(`[releases] desktop fetch failed: ${(err as Error).message}`);
    }

    // 移动端可选（404/503 不阻塞）
    try {
      const m = await fetchRepoReleases(MOBILE_REPO);
      mobile = { latest: m.latest };
    } catch (err) {
      console.warn(`[releases] mobile fetch failed (non-fatal): ${(err as Error).message}`);
      mobile = { latest: null };
    }

    if (liveOk && desktop) {
      const data: ReleasesData = {
        desktop,
        mobile: mobile ?? { latest: null },
        fetchedAt,
        source: "live",
      };
      // 仅在 build / production 写盘（dev 模式频繁写盘没必要）
      if (process.env.NODE_ENV !== "development") {
        await writeCache(data);
      }
      return data;
    }

    // Fallback: 用 cache
    const cached = await readCache();
    if (cached) {
      console.warn(
        `[releases] WARN: GitHub API unreachable, using cached releases (last updated: ${cached.fetchedAt})`,
      );
      return { ...cached, source: "cache" };
    }

    // 无 cache 也无 live —— 返回空骨架，UI 显示 placeholder
    console.warn("[releases] WARN: no live data and no cache; rendering empty.");
    return {
      desktop: { latest: null, history: [] },
      mobile: { latest: null },
      fetchedAt,
      source: "empty",
    };
  })();
  return memo;
}

// ---------- UI 友好辅助 ----------
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "—";
  const units = ["B", "KB", "MB", "GB"];
  let i = 0;
  let n = bytes;
  while (n >= 1024 && i < units.length - 1) {
    n /= 1024;
    i++;
  }
  return `${n.toFixed(n >= 100 || i === 0 ? 0 : 1)} ${units[i]}`;
}

export function formatPublishedAt(iso: string, lang: "zh" | "en" = "zh"): string {
  if (!iso) return "—";
  try {
    const d = new Date(iso);
    if (lang === "zh") {
      return d.toISOString().slice(0, 10);
    }
    return d.toLocaleDateString("en-US", {
      year: "numeric",
      month: "short",
      day: "numeric",
    });
  } catch {
    return iso.slice(0, 10);
  }
}

/**
 * 桌面端按平台分组资产，单个槽位优先 arm64（macOS）或 x64。
 * 用于主页 Download 卡片摘要。
 */
export function groupDesktopAssetsByPlatform(
  release: NormalizedRelease | null,
): Record<"windows" | "macos" | "linux", ReleaseAsset[]> {
  const out = {
    windows: [] as ReleaseAsset[],
    macos: [] as ReleaseAsset[],
    linux: [] as ReleaseAsset[],
  };
  if (!release) return out;
  for (const a of release.assets) {
    if (a.platform === "windows") out.windows.push(a);
    else if (a.platform === "macos") out.macos.push(a);
    else if (a.platform === "linux") out.linux.push(a);
  }
  return out;
}

/** 收集未识别 / 校验和等 other 资产（出现在 /download 底部） */
export function collectOtherAssets(release: NormalizedRelease | null): ReleaseAsset[] {
  return release?.assets.filter((a) => a.platform === "other") ?? [];
}
