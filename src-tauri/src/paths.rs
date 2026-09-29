//! 本地配置文件放在哪（登录会话、测试版自带 key 的配置）。
//!
//! - 桌面端：`dirs::config_dir()/soul-lantern/`，和以前完全一样，老用户的登录状态不受影响。
//! - 安卓 / iOS：`dirs` 在移动端拿不到配置目录（返回 None），会话存不下来、每次打开都得
//!   重新登录。所以移动端改用 Tauri 给的应用私有目录，启动时在 `lib.rs` 的 setup 里设置一次。
use std::path::PathBuf;

#[cfg(mobile)]
static MOBILE_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// 移动端启动时调用一次。桌面端不用调。
#[cfg(mobile)]
pub fn init_mobile(dir: PathBuf) {
    let _ = MOBILE_DIR.set(dir);
}

/// 本软件的配置目录。拿不到就是 None（调用方按「没有保存过」处理）。
pub fn config_dir() -> Option<PathBuf> {
    #[cfg(mobile)]
    {
        MOBILE_DIR.get().cloned()
    }
    #[cfg(not(mobile))]
    {
        dirs::config_dir().map(|d| d.join("soul-lantern"))
    }
}
