//! 管理页的 Tauri 命令。薄薄一层转发，判断权全在服务端。
//!
//! 这一层刻意不做任何"这个人是不是管理员"的判断——那个判断在服务端的
//! 会话上（`Session.admin_verified`）。客户端这边即使把界面上的管理入口
//! 强行显示出来，每一个调用照样会被服务端 404 挡回来。界面上的
//! `adminVerified` 只是用来决定"要不要画这个入口"，不是安全边界。

use crate::remote;
use crate::session;

#[tauri::command]
pub async fn admin_users() -> Result<serde_json::Value, String> {
    remote::admin_users().await
}

#[tauri::command]
pub async fn admin_lookup(query: String) -> Result<serde_json::Value, String> {
    remote::admin_lookup(query.trim()).await
}

#[tauri::command]
pub async fn admin_adjust_balance(query: String, delta: i64) -> Result<serde_json::Value, String> {
    if delta == 0 {
        return Err("变动量不能是 0。".to_string());
    }
    remote::admin_adjust_balance(query.trim(), delta).await
}

#[tauri::command]
pub async fn admin_delete_user(query: String, confirm: String) -> Result<serde_json::Value, String> {
    remote::admin_delete_user(query.trim(), confirm.trim()).await
}

#[tauri::command]
pub async fn admin_get_env() -> Result<serde_json::Value, String> {
    remote::admin_get_env().await
}

#[tauri::command]
pub async fn admin_set_env(
    key: String,
    value: String,
    confirm: Option<String>,
) -> Result<serde_json::Value, String> {
    remote::admin_set_env(key.trim(), &value, confirm.as_deref().map(str::trim)).await
}

#[tauri::command]
pub async fn admin_get_policy() -> Result<serde_json::Value, String> {
    remote::admin_get_policy().await
}

/// 保存价格配置。
///
/// 前端传过来的是一段 JSON **文本**（管理页里那是个代码编辑框），这里先在
/// 本地解析一次再发出去。本地解析不了的话根本没必要跑一趟网络，而且能给出
/// "第几行第几列"这种服务端给不了的错误位置。
#[tauri::command]
pub async fn admin_set_policy(policy_json: String) -> Result<serde_json::Value, String> {
    let parsed: serde_json::Value = serde_json::from_str(&policy_json)
        .map_err(|e| format!("这段 JSON 有语法错误（第 {} 行第 {} 列）：{e}", e.line(), e.column()))?;
    remote::admin_set_policy(parsed).await
}

/// 以某人身份登录。
///
/// **这会把当前的管理员会话顶掉**：本地只存得下一个 token，换上目标用户的
/// 之后，管理员那条会话虽然在服务端还活着，但客户端已经找不回它了，
/// 要回去只能重新登录 + 重新输 token。界面上必须先说清楚这件事再让点。
#[tauri::command]
pub async fn admin_impersonate(query: String) -> Result<String, String> {
    let r = remote::admin_impersonate(query.trim()).await?;
    let token = r
        .get("token")
        .and_then(|t| t.as_str())
        .ok_or_else(|| "服务器没有返回可用的会话。".to_string())?;
    let expires_at = r.get("expiresAt").and_then(serde_json::Value::as_u64).unwrap_or(0);
    let username =
        r.get("username").and_then(|u| u.as_str()).unwrap_or("那个用户").to_string();
    session::save(token, expires_at);
    Ok(username)
}

#[tauri::command]
pub async fn admin_health() -> Result<serde_json::Value, String> {
    remote::admin_health().await
}

/// 重启服务端。
///
/// 服务端是用非零退出码触发 systemd 重启的，所以这个请求**多半拿不到正常
/// 响应**——进程在 300ms 后就没了，连接会被切断。因此"连接中断"在这里
/// 恰恰是成功的信号，不能当成失败报给用户。
#[tauri::command]
pub async fn admin_restart() -> Result<(), String> {
    match remote::admin_restart().await {
        Ok(_) => Ok(()),
        Err(e) if e.contains("无法连接服务器") || e.contains("解析服务器响应失败") => Ok(()),
        Err(e) => Err(e),
    }
}
