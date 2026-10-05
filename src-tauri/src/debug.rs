use serde::Serialize;
use soul_lantern_mc_debug::{Cancellation, Engine, ProcessControl, SessionGate, Validation};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};

struct State {
    gate: Mutex<SessionGate>,
    engine: Mutex<Option<Engine>>,
    process: ProcessControl,
    cache: Mutex<PathBuf>,
    progress: Arc<Mutex<String>>,
}
static STATE: OnceLock<State> = OnceLock::new();

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugState {
    enabled: bool,
    busy: bool,
    cache_dir: String,
    progress: String,
}

fn default_cache() -> PathBuf {
    #[cfg(windows)]
    if PathBuf::from("D:\\").is_dir() {
        return PathBuf::from("D:\\soul-lantern-build-cache\\soul-lantern-debug");
    }
    dirs::cache_dir()
        .or_else(crate::paths::config_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("soul-lantern-debug")
}
fn cache_config() -> Option<PathBuf> {
    crate::paths::config_dir().map(|p| p.join("debug-cache.json"))
}
fn state() -> &'static State {
    STATE.get_or_init(|| {
        let cache = cache_config()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<PathBuf>(&s).ok())
            .filter(|p| p.is_absolute())
            .unwrap_or_else(default_cache);
        State {
            gate: Mutex::new(SessionGate::default()),
            engine: Mutex::new(None),
            process: ProcessControl::default(),
            cache: Mutex::new(cache),
            progress: Arc::new(Mutex::new(String::new())),
        }
    })
}

pub fn shutdown() {
    if let Some(s) = STATE.get() {
        s.gate.lock().unwrap().disable();
        s.process.stop();
        if let Ok(mut engine) = s.engine.try_lock() {
            *engine = None;
        }
        *s.progress.lock().unwrap() = "调试模式已关闭。".into();
    }
}

async fn admin_session() -> Result<String, String> {
    let token = crate::session::token().ok_or("请登录并完成管理员认证。")?;
    if crate::remote::admin_health().await.is_err() {
        shutdown();
        return Err("无法验证管理员会话，请检查网络或重新完成管理员认证。".into());
    }
    if crate::session::token().as_deref() != Some(&token) {
        shutdown();
        return Err("登录会话已改变，请重新开启调试。".into());
    }
    Ok(token)
}

#[tauri::command]
pub fn debug_state() -> DebugState {
    let s = state();
    let enabled =
        crate::session::token().is_some_and(|t| s.gate.lock().unwrap().permit(&t).is_ok());
    if !enabled {
        shutdown();
    }
    DebugState {
        enabled,
        busy: s.engine.try_lock().is_err(),
        cache_dir: s.cache.lock().unwrap().to_string_lossy().into(),
        progress: s.progress.lock().unwrap().clone(),
    }
}

#[tauri::command]
pub async fn debug_enable(cache_dir: String, eula_accepted: bool) -> Result<DebugState, String> {
    #[cfg(mobile)]
    return Err("本机调试仅支持桌面系统。".into());
    if !eula_accepted {
        return Err("启动官方 Minecraft 服务端前，请阅读并同意 Minecraft EULA。".into());
    }
    let token = admin_session().await?;
    let selected = PathBuf::from(cache_dir.trim());
    if !selected.is_absolute() {
        return Err("请选择一个绝对路径作为调试缓存目录。".into());
    }
    // 所有下载和临时世界都放到自有子目录，避免清理用户选中的目录本身。
    let cache = if selected
        .file_name()
        .is_some_and(|n| n == "soul-lantern-debug")
    {
        selected
    } else {
        selected.join("soul-lantern-debug")
    };
    let s = state();
    let mut engine = s
        .engine
        .try_lock()
        .map_err(|_| "上次操作仍在结束，请稍后再开启。")?;
    shutdown();
    std::fs::create_dir_all(&cache).map_err(|e| format!("无法写入调试缓存目录：{e}"))?;
    if let Some(path) = cache_config() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(
            path,
            serde_json::to_string(&cache).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    *s.cache.lock().unwrap() = cache.clone();
    *engine = Some(Engine::new(cache, s.process.clone()));
    s.gate.lock().unwrap().enable(token);
    *s.progress.lock().unwrap() = "已开启。首次点击验证时准备 JDK 21、JDK 25 与服务端。".into();
    drop(engine);
    Ok(debug_state())
}

#[tauri::command]
pub fn debug_disable() -> DebugState {
    shutdown();
    debug_state()
}

#[tauri::command]
pub async fn debug_validate(version: String, command: String) -> Result<Validation, String> {
    let token = admin_session().await?;
    let cancel: Cancellation = state().gate.lock().unwrap().permit(&token)?;
    let progress = state().progress.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        cancel.check()?;
        let mut engine = state()
            .engine
            .try_lock()
            .map_err(|_| "另一条指令正在验证，请等待完成。")?;
        let engine = engine.as_mut().ok_or("调试模式已关闭。")?;
        let result = engine.validate(&version, &command, &cancel, &move |text| {
            *progress.lock().unwrap() = text.into()
        });
        cancel.check()?;
        result
    })
    .await
    .map_err(|e| format!("调试任务失败：{e}"))?;
    *state().progress.lock().unwrap() = match &result {
        Ok(r) => format!("Minecraft {}：{}", r.version, r.message),
        Err(e) => e.clone(),
    };
    result
}
