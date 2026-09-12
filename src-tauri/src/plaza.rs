//! 万灯集的 Tauri 命令。
//!
//! 全部原样转发，`serde_json::Value` 进出。理由见 remote.rs 里万灯集那一段
//! 的注释：客户端一个字段都不参与计算，中间加一层 Rust 结构体只会变成
//! "每加一个字段要在三个地方各写一遍"。

use crate::remote;

#[tauri::command]
pub async fn plaza_categories() -> Result<serde_json::Value, String> {
    remote::plaza_categories().await
}

/// 列表。`query` 是已经拼好的 query string（`kind=ai&sort=hot` 这种），
/// 前端拼比在这里定义一堆 Option 参数省事得多。
#[tauri::command]
pub async fn plaza_list(query: Option<String>) -> Result<serde_json::Value, String> {
    remote::plaza_list(query.as_deref().unwrap_or("")).await
}

#[tauri::command]
pub async fn plaza_get(id: String) -> Result<serde_json::Value, String> {
    remote::plaza_get(&id).await
}

#[tauri::command]
pub async fn plaza_publish(
    kind: String,
    title: String,
    icon: Option<String>,
    category: String,
    detail_md: String,
    payload: String,
) -> Result<serde_json::Value, String> {
    remote::plaza_publish(serde_json::json!({
        "kind": kind,
        "title": title.trim(),
        "icon": icon,
        "category": category,
        "detailMd": detail_md,
        "payload": payload,
    }))
    .await
}

#[tauri::command]
pub async fn plaza_delete(id: String) -> Result<serde_json::Value, String> {
    remote::plaza_delete(&id).await
}

#[tauri::command]
pub async fn plaza_like(id: String) -> Result<serde_json::Value, String> {
    remote::plaza_like(&id).await
}

#[tauri::command]
pub async fn plaza_favorite(id: String) -> Result<serde_json::Value, String> {
    remote::plaza_favorite(&id).await
}

/// 记一次下载量。**失败不返回错误**：用户真正要的东西（payload）在详情里
/// 已经拿到了，这只是个计数。为了一个计数失败去弹一个"下载失败"，
/// 会让用户以为模板没拿到。
#[tauri::command]
pub async fn plaza_download(id: String) -> Result<(), String> {
    let _ = remote::plaza_download(&id).await;
    Ok(())
}

#[tauri::command]
pub async fn plaza_comment(id: String, body: String) -> Result<serde_json::Value, String> {
    remote::plaza_comment(&id, body.trim()).await
}

#[tauri::command]
pub async fn plaza_delete_comment(id: String, comment_id: String) -> Result<serde_json::Value, String> {
    remote::plaza_delete_comment(&id, &comment_id).await
}
