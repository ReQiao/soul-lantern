/**
 * 贡献者计划的数据收集（前端这一侧）。
 *
 * 只有「已登录 + 贡献者等级 ≥ 1」才会真的发出去；没加入的人这里什么都不发。
 * 服务端也会再查一遍等级，不够直接拒收。
 *
 * 收什么（和 EULA 第七节、设置页的说明一一对应，改这里要同步改那两处）：
 * - 电脑架构信息：系统版本、CPU、内存、屏幕尺寸、软件版本（见 src-tauri 的
 *   telemetry_system_info），不含主机名 / 用户名 / 硬件序列号。
 * - 出错时的操作记录：最近 40 次点击了哪个按钮 / 标签（按钮上的字），以及出错信息。
 *   **不记输入框里打的字。**
 * - 高级贡献者的 AI 提示词和输出由服务端自己记，这里不管。
 */
import { invoke } from "@tauri-apps/api/core";
import type { App } from "vue";
import { auth, desktop } from "./auth";

const MAX_CRUMBS = 40;
const MAX_ERRORS_PER_RUN = 20;
const DEDUPE_MS = 60_000;

const crumbs: string[] = [];
const recentErrors: string[] = [];
const lastSent = new Map<string, number>();
let sentThisRun = 0;
let sessionSent = false;
let systemInfo: Record<string, unknown> | null = null;

function stamp(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

/** 记一条操作轨迹。 */
export function crumb(text: string) {
  crumbs.push(`${stamp()} ${text}`);
  if (crumbs.length > MAX_CRUMBS) crumbs.shift();
}

function enabled(): boolean {
  return desktop && auth.value.loggedIn && auth.value.contributorLevel >= 1;
}

async function getSystemInfo(): Promise<Record<string, unknown>> {
  if (!systemInfo) {
    try {
      systemInfo = await invoke<Record<string, unknown>>("telemetry_system_info");
    } catch {
      systemInfo = {};
    }
  }
  return {
    ...systemInfo,
    screen: `${window.screen.width}x${window.screen.height}`,
    viewport: `${window.innerWidth}x${window.innerHeight}`,
    dpr: window.devicePixelRatio,
    language: navigator.language,
    webview: navigator.userAgent,
  };
}

/**
 * 报一个错误。没加入贡献者计划时只留在本地（「复制诊断信息」里能看到），不发。
 * 同一条错误一分钟内只报一次，一次运行最多报 20 条。
 */
export function reportError(context: string, message: string) {
  const msg = String(message).slice(0, 2000);
  recentErrors.push(`${stamp()} [${context}] ${msg}`);
  if (recentErrors.length > 10) recentErrors.shift();

  if (!enabled() || sentThisRun >= MAX_ERRORS_PER_RUN) return;
  const key = `${context}|${msg}`;
  const now = Date.now();
  if (now - (lastSent.get(key) ?? 0) < DEDUPE_MS) return;
  lastSent.set(key, now);
  sentThisRun += 1;

  void (async () => {
    const data = { context, message: msg, crumbs: [...crumbs], system: await getSystemInfo() };
    await invoke("telemetry_send", { kind: "error", data }).catch(() => {});
  })();
}

/** 每次运行、加入计划后发一次系统信息（知道大家在什么环境下用，才知道该兼容什么）。 */
export function maybeSendSession() {
  if (sessionSent || !enabled()) return;
  sessionSent = true;
  void (async () => {
    await invoke("telemetry_send", { kind: "session", data: { system: await getSystemInfo() } }).catch(() => {});
  })();
}

/** 「复制诊断信息」用的文本。不管加没加入计划都能用——是用户自己复制、自己决定发给谁。 */
export async function diagnosticsText(): Promise<string> {
  const sys = await getSystemInfo();
  const lines = [
    `灵魂灯笼 ${sys.appVersion ?? ""}（测试版）`,
    `系统：${sys.osVersion || sys.os || ""} ${sys.arch ?? ""}`,
    `CPU：${sys.cpu ?? ""}（${sys.cpuCores ?? "?"} 线程），内存 ${sys.memoryMb ?? "?"} MB`,
    `屏幕：${sys.screen}，窗口 ${sys.viewport}，缩放 ${sys.dpr}`,
    `用户名：${auth.value.loggedIn ? auth.value.username : "（未登录）"}`,
    "",
    "最近的错误：",
    ...(recentErrors.length ? recentErrors : ["（无）"]),
    "",
    "最近的操作：",
    ...(crumbs.length ? crumbs.slice(-15) : ["（无）"]),
  ];
  return lines.join("\n");
}

function labelOf(el: Element): string {
  const raw = el.getAttribute("aria-label") || el.getAttribute("title") || el.textContent || "";
  return raw.replace(/\s+/g, " ").trim().slice(0, 30);
}

/** 在 main.ts 里调一次。 */
export function installTelemetry(app: App) {
  document.addEventListener(
    "click",
    (e) => {
      const el = (e.target as Element | null)?.closest?.("button, [role='tab'], a, summary, label");
      if (!el) return;
      const label = labelOf(el);
      if (label) crumb(`点击「${label}」`);
    },
    true,
  );
  window.addEventListener("error", (e) => {
    reportError("window.error", `${e.message} @ ${e.filename}:${e.lineno}:${e.colno}`);
  });
  window.addEventListener("unhandledrejection", (e) => {
    const r = e.reason;
    reportError("unhandledrejection", r instanceof Error ? `${r.message}\n${r.stack ?? ""}` : String(r));
  });
  app.config.errorHandler = (err, _instance, info) => {
    console.error(err);
    reportError(`vue:${info}`, err instanceof Error ? `${err.message}\n${err.stack ?? ""}` : String(err));
  };
}
