//! 窗口控制。目前只有一件事：F11 全屏。
//!
//! # 为什么走自己的 command，而不是前端直接调 `@tauri-apps/api/window`
//!
//! Tauri v2 的 `core:default` 权限集里有 `core:window:allow-is-fullscreen`
//! （读），但**没有** `allow-set-fullscreen`（写）。前端要自己切就得往
//! `capabilities/default.json` 里加一条权限，等于给整个 webview 开了一个能改
//! 窗口状态的口子——而这个应用的前端将来是要开源的，被注入脚本能做的事越少越好。
//!
//! 走 Rust 侧的 command 不需要动 capabilities：权限检查针对的是 core 插件暴露
//! 给前端的那些接口，自己写的 command 直接拿 `tauri::Window` 句柄。
use tauri::Window;

/// 切换全屏。返回切换**之后**的状态，前端拿它更新提示文案。
#[tauri::command]
pub fn window_toggle_fullscreen(window: Window) -> Result<bool, String> {
    let next = !window.is_fullscreen().map_err(|e| e.to_string())?;
    window.set_fullscreen(next).map_err(|e| e.to_string())?;
    // 退出全屏之后焦点有时会掉在 webview 外面，键盘事件收不到——下一次 F11
    // 就没反应了，看起来像"全屏卡住了"。这里主动要回来。
    let _ = window.set_focus();
    Ok(next)
}

/// 当前是不是全屏。启动时前端要用它对齐初始状态。
#[tauri::command]
pub fn window_is_fullscreen(window: Window) -> Result<bool, String> {
    window.is_fullscreen().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    /// 这两个 command 都要真实的 `tauri::Window` 句柄，单测里造不出来
    /// （造得出来的话就是在测 tauri 自己）。真正的验证在手动一步：
    /// 打开应用按 F11。这里留个空壳说明为什么没有测试，免得下次有人以为是漏了。
    #[test]
    fn covered_manually_not_by_unit_test() {}
}
