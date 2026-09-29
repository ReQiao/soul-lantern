//! 贡献者计划（客户端这一侧）。规则和奖励都在服务端（soul-lantern-server 的
//! contributor.rs），这里只负责：改等级、收集系统信息、把错误记录上报。
//!
//! 上报只在「已登录 + 服务端记录的等级 ≥ 1」时才会真正发出去；服务端也会再查一遍，
//! 等级不够直接拒收。高级贡献者的 AI 提示词和输出由服务端自己记，客户端不传。

use crate::{remote, session};
use serde::Serialize;

#[tauri::command]
pub async fn contributor_set(level: u8, dry_run: bool) -> Result<remote::ContributorView, String> {
    remote::contributor_set(level, dry_run).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub app_version: &'static str,
    pub os: &'static str,
    pub arch: &'static str,
    /// 例如 "Windows 11 (26100)"。拿不到就是空。
    pub os_version: String,
    pub cpu: String,
    pub cpu_cores: usize,
    pub memory_mb: u64,
}

/// 贡献者上报里的「电脑架构信息」。不含任何能认出人的东西（主机名、用户名、
/// 硬件序列号、IP 都不收）。
#[tauri::command]
pub fn telemetry_system_info() -> SystemInfo {
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();
    SystemInfo {
        app_version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        os_version: System::long_os_version().unwrap_or_default(),
        cpu: sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default(),
        cpu_cores: sys.cpus().len(),
        memory_mb: sys.total_memory() / 1024 / 1024,
    }
}

/// 上报一条。没登录就静默丢掉——上报是附带的，永远不该弹错误打扰用户。
#[tauri::command]
pub async fn telemetry_send(kind: String, data: serde_json::Value) -> Result<(), ()> {
    if session::token().is_none() {
        return Ok(());
    }
    let _ = remote::telemetry_upload(&kind, &data).await;
    Ok(())
}
