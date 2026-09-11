/**
 * F11 全屏。
 *
 * # 为什么要自己接，浏览器不是本来就有 F11 吗
 *
 * 那是浏览器**外壳**的功能，不是网页的。Tauri 的 webview 没有那层外壳，
 * 按 F11 什么都不会发生。所以要自己收键、自己调窗口 API。
 *
 * # 为什么走 invoke 而不是 `@tauri-apps/api/window`
 *
 * 见 src-tauri/src/window.rs 顶部：`core:default` 没给 `allow-set-fullscreen`，
 * 前端要自己切就得往 capabilities 里加权限，等于给 webview 开一个能改窗口状态
 * 的口子。走自己的 command 不用动权限。
 *
 * # 浏览器里不拦
 *
 * `npm run dev` 在真浏览器里跑时，F11 是浏览器自己的全屏，比我们做的好用。
 * 那种情况下不要 preventDefault——拦了反而把好用的那个弄没了。
 */
import { invoke, isTauri } from "@tauri-apps/api/core";
import { ref } from "vue";

/** 当前是不是全屏。给界面上想显示提示的地方用，现在没人用，留着不亏。 */
export const isFullscreen = ref(false);

export async function toggleFullscreen(): Promise<void> {
  if (!isTauri()) return;
  try {
    isFullscreen.value = await invoke<boolean>("window_toggle_fullscreen");
  } catch {
    // 切不了就算了，不能让一个快捷键把整个界面搞出错误提示
  }
}

/**
 * 装上 F11 监听。整个应用调用一次，返回卸载函数。
 *
 * 用 capture 阶段（第三个参数 true）：输入框、富文本编辑器、弹窗里都有自己的
 * keydown 处理，其中几处会 stopPropagation。挂在冒泡阶段的话，光标在物品名称
 * 那个富文本里时按 F11 就没反应了——而"我正在打字，顺手全屏一下"恰恰是最常见
 * 的用法。
 */
export function installFullscreen(): () => void {
  if (typeof document === "undefined") return () => {};

  const onKey = (e: KeyboardEvent) => {
    if (e.key !== "F11" || e.repeat) return;
    // 浏览器里让原生的 F11 干活，我们不掺和
    if (!isTauri()) return;
    e.preventDefault();
    void toggleFullscreen();
  };

  document.addEventListener("keydown", onKey, true);

  // 启动时对齐一次真实状态：窗口可能是以全屏启动的（比如上次退出时是全屏，
  // 或者系统级的窗口管理器给恢复成了全屏）。不对齐的话第一次按 F11 会反着来。
  if (isTauri()) {
    invoke<boolean>("window_is_fullscreen")
      .then((v) => (isFullscreen.value = v))
      .catch(() => {});
  }

  return () => document.removeEventListener("keydown", onKey, true);
}
