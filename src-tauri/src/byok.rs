//! 【测试版功能，正式商业化时服务端会关掉对应接口】用户自带 API key。
//!
//! 流程：本地拿用户自己的 key 直接调用户自己选的大模型接口（兼容 OpenAI 格式：
//! OpenAI / DeepSeek / 通义千问 / 各种中转都行）→ 拿到 AI 输出的那段 JSON →
//! 交给服务端 `/v1/byok/build` 用服务端的构建器转成指令。
//!
//! **key 从不经过我们的服务器**：一是用户放心填；二是 OpenAI 这类接口在国内要走代理，
//! 服务器上走不通，只能从用户电脑上发。所以这里用的是一个**普通的** HTTPS 客户端
//! （公共 CA + 系统代理），不是 remote.rs 那个只认自家证书的锁定客户端。
//!
//! key 的存储：`dirs::config_dir()/soul-lantern/byok.json`。
//! - Windows：用系统 DPAPI（CryptProtectData）加密后再写文件。密钥绑定当前 Windows
//!   账号，文件被拷到别的电脑 / 别的账号下解不开，同步到网盘也不怕。
//! - 其它系统：明文写文件，权限设成只有自己能读（0600）。
//!
//! 【防不住的东西，说在前面】同一个 Windows 账号下运行的程序（比如 Cheat Engine、
//! 或者恶意软件）能读这个进程的内存，也能调用 DPAPI 解密——任何软件都一样，
//! 本地同权限的攻击者本来就赢了。这里防的是文件被复制走 / 被顺手读走。
//! 自己写一个编进软件里的密钥来"加密"是没有意义的：客户端是开源的，谁都能解。

use crate::ai::{AiResponse, AiUsage, ChatTurn};
use crate::remote;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ---------------------------------------------------------------- 存储

#[derive(Serialize, Deserialize, Default)]
struct Stored {
    endpoint: String,
    model: String,
    /// 加密（Windows）或原样（其它系统）的 key，base64。空 = 没存。
    #[serde(default)]
    key: String,
    /// "dpapi" 或 "plain"。读的时候按它解，换了系统也不会解错。
    #[serde(default)]
    protection: String,
}

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("soul-lantern").join("byok.json"))
}

fn load() -> Stored {
    config_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save(st: &Stored) -> Result<(), String> {
    let path = config_path().ok_or("找不到配置目录")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建配置目录失败：{e}"))?;
    }
    let text = serde_json::to_string_pretty(st).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("保存失败：{e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

fn protect(plain: &str) -> Result<(String, &'static str), String> {
    use base64::Engine;
    #[cfg(windows)]
    {
        let sealed = dpapi::protect(plain.as_bytes())?;
        Ok((b64().encode(sealed), "dpapi"))
    }
    #[cfg(not(windows))]
    {
        Ok((b64().encode(plain.as_bytes()), "plain"))
    }
}

fn unprotect(st: &Stored) -> Result<String, String> {
    use base64::Engine;
    if st.key.is_empty() {
        return Err("还没有填 API key。".to_string());
    }
    let raw = b64().decode(&st.key).map_err(|_| "保存的 key 已损坏，请重新填写。".to_string())?;
    let bytes = match st.protection.as_str() {
        #[cfg(windows)]
        "dpapi" => dpapi::unprotect(&raw)?,
        "plain" => raw,
        _ => return Err("保存的 key 无法在这台电脑 / 这个账号上解开，请重新填写。".to_string()),
    };
    String::from_utf8(bytes).map_err(|_| "保存的 key 已损坏，请重新填写。".to_string())
}

#[cfg(windows)]
mod dpapi {
    //! Windows DPAPI：CryptProtectData / CryptUnprotectData，作用域是当前用户。
    use windows_sys::Win32::Foundation::{LocalFree, HLOCAL};
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    fn run(input: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
        let mut in_blob = CRYPT_INTEGER_BLOB { cbData: input.len() as u32, pbData: input.as_ptr() as *mut u8 };
        let mut out_blob = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        // SAFETY: 输入 blob 指向活着的切片；输出 blob 由系统分配，下面拷出来后用 LocalFree 释放。
        let ok = unsafe {
            if encrypt {
                CryptProtectData(
                    &mut in_blob,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut out_blob,
                )
            } else {
                CryptUnprotectData(
                    &mut in_blob,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut out_blob,
                )
            }
        };
        if ok == 0 {
            return Err(if encrypt {
                "加密 key 失败。".to_string()
            } else {
                "保存的 key 无法在这台电脑 / 这个账号上解开，请重新填写。".to_string()
            });
        }
        // SAFETY: 系统保证 out_blob 指向 cbData 字节的有效内存。
        let out = unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec() };
        unsafe { LocalFree(out_blob.pbData as HLOCAL) };
        Ok(out)
    }

    pub fn protect(plain: &[u8]) -> Result<Vec<u8>, String> {
        run(plain, true)
    }

    pub fn unprotect(sealed: &[u8]) -> Result<Vec<u8>, String> {
        run(sealed, false)
    }
}

// ---------------------------------------------------------------- 校验

/// 接口地址统一成 `https://host/v1` 这种"基址"：去掉结尾的 `/` 和用户顺手粘进来的
/// `/chat/completions`。只允许 https；http 只放行本机（Ollama 这类本地模型）。
fn normalize_endpoint(raw: &str) -> Result<String, String> {
    let mut e = raw.trim().trim_end_matches('/').to_string();
    if let Some(stripped) = e.strip_suffix("/chat/completions") {
        e = stripped.trim_end_matches('/').to_string();
    }
    let lower = e.to_ascii_lowercase();
    let local = ["http://localhost", "http://127.0.0.1", "http://[::1]"]
        .iter()
        .any(|p| lower == *p || lower.starts_with(&format!("{p}:")) || lower.starts_with(&format!("{p}/")));
    if !(lower.starts_with("https://") || local) {
        return Err("接口地址要以 https:// 开头（本机模型可以用 http://localhost）。".to_string());
    }
    if e.len() <= "https://".len() {
        return Err("接口地址不完整。".to_string());
    }
    Ok(e)
}

/// 本地先把格式明显不对的 key 挡掉，不发请求。
fn validate_key(key: &str) -> Result<(), String> {
    let k = key.trim();
    if k.is_empty() {
        return Err("API key 是空的。".to_string());
    }
    if !k.chars().all(|c| c.is_ascii_graphic()) {
        return Err("API key 里有空格或中文等非法字符，是不是复制多了？".to_string());
    }
    if k.len() < 8 || k.len() > 512 {
        return Err("API key 长度不对，检查一下是否复制完整。".to_string());
    }
    Ok(())
}

fn key_hint(key: &str) -> String {
    let tail: String = key.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    format!("…{tail}")
}

// ---------------------------------------------------------------- 命令

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByokConfigView {
    pub endpoint: String,
    pub model: String,
    pub has_key: bool,
    /// key 的最后 4 位，界面上让用户认出存的是哪一把。
    pub key_hint: String,
}

fn view(st: &Stored) -> ByokConfigView {
    let hint = unprotect(st).map(|k| key_hint(&k)).unwrap_or_default();
    ByokConfigView {
        endpoint: st.endpoint.clone(),
        model: st.model.clone(),
        has_key: !hint.is_empty(),
        key_hint: hint,
    }
}

#[tauri::command]
pub fn byok_get_config() -> ByokConfigView {
    view(&load())
}

/// 保存配置。`key` 为 None / 空时保留原来的 key（界面上不回显 key，
/// 用户只改模型名时不用重新粘一遍）。
#[tauri::command]
pub fn byok_save_config(endpoint: String, model: String, key: Option<String>) -> Result<ByokConfigView, String> {
    let endpoint = normalize_endpoint(&endpoint)?;
    let model = model.trim().to_string();
    if model.is_empty() {
        return Err("模型名不能为空。".to_string());
    }
    let mut st = load();
    st.endpoint = endpoint;
    st.model = model;
    if let Some(k) = key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()) {
        validate_key(&k)?;
        let (sealed, how) = protect(&k)?;
        st.key = sealed;
        st.protection = how.to_string();
    }
    save(&st)?;
    Ok(view(&st))
}

#[tauri::command]
pub fn byok_clear_key() -> Result<ByokConfigView, String> {
    let mut st = load();
    st.key.clear();
    st.protection.clear();
    save(&st)?;
    Ok(view(&st))
}

fn http() -> Result<reqwest::Client, String> {
    // 普通客户端：公共 CA（reqwest 的 rustls-tls 自带 webpki 根证书）+ 系统代理
    // （Windows / macOS 的系统代理设置，Clash 这类工具开"系统代理"就能用）。
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败：{e}"))
}

fn describe_net_err(e: reqwest::Error) -> String {
    if e.is_timeout() {
        "请求超时。如果用的是国外接口，检查代理是否开着。".to_string()
    } else {
        format!("连不上接口：{e}。如果用的是国外接口，检查代理是否开着。")
    }
}

/// 测试连接：拿 key 请求一次 `{endpoint}/models`（不花钱）。
/// 不是所有服务商都有这个接口，拿到 404 就说"没法验证，但可以直接试着生成"。
#[tauri::command]
pub async fn byok_test() -> Result<String, String> {
    let st = load();
    let endpoint = normalize_endpoint(&st.endpoint)?;
    let key = unprotect(&st)?;
    let resp = http()?
        .get(format!("{endpoint}/models"))
        .bearer_auth(&key)
        .send()
        .await
        .map_err(describe_net_err)?;
    match resp.status().as_u16() {
        200..=299 => Ok("连接成功，key 有效。".to_string()),
        401 | 403 => Err("key 无效，或者没有权限。".to_string()),
        404 | 405 => Ok("这个服务商不支持验证接口，没法提前确认 key，直接试着生成吧。".to_string()),
        429 => Err("请求太频繁或额度用完了（429）。".to_string()),
        s => Err(format!("接口返回了 {s}，检查一下接口地址。")),
    }
}

#[derive(Deserialize)]
struct UpstreamResp {
    choices: Vec<UpstreamChoice>,
    #[serde(default)]
    usage: Option<UpstreamUsage>,
}
#[derive(Deserialize)]
struct UpstreamChoice {
    message: UpstreamMessage,
}
#[derive(Deserialize)]
struct UpstreamMessage {
    #[serde(default)]
    content: Option<String>,
}
#[derive(Deserialize)]
struct UpstreamUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    #[serde(default)]
    total_tokens: u32,
}

fn failure(error: String) -> AiResponse {
    AiResponse {
        ok: false,
        commands: Vec::new(),
        loop_commands: Vec::new(),
        failures: Vec::new(),
        explanation: String::new(),
        error: Some(error),
        usage: None,
        raw_content: None,
        balance: None,
    }
}

async fn call_own_model(system_prompt: &str, user_text: &str, history: &[ChatTurn]) -> Result<(String, Option<AiUsage>), String> {
    let st = load();
    let endpoint = normalize_endpoint(&st.endpoint)?;
    let key = unprotect(&st)?;

    let mut messages = vec![serde_json::json!({ "role": "system", "content": system_prompt })];
    for t in history {
        messages.push(serde_json::json!({ "role": t.role, "content": t.content }));
    }
    messages.push(serde_json::json!({ "role": "user", "content": user_text }));

    let resp = http()?
        .post(format!("{endpoint}/chat/completions"))
        .bearer_auth(&key)
        .json(&serde_json::json!({
            "model": st.model,
            "messages": messages,
            "response_format": { "type": "json_object" },
            "temperature": 0.2,
        }))
        .send()
        .await
        .map_err(describe_net_err)?;

    let status = resp.status();
    if !status.is_success() {
        let detail = resp.text().await.unwrap_or_default();
        let detail: String = detail.chars().take(300).collect();
        return Err(match status.as_u16() {
            401 | 403 => "key 无效，或者没有权限。".to_string(),
            404 => format!("接口地址或模型名不对（404）：{detail}"),
            429 => "请求太频繁或额度用完了（429）。".to_string(),
            s => format!("接口返回了 {s}：{detail}"),
        });
    }
    let body: UpstreamResp = resp.json().await.map_err(|e| format!("接口返回的格式看不懂：{e}"))?;
    let content = body
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .filter(|c| !c.trim().is_empty())
        .ok_or("模型没有返回内容。")?;
    let usage = body.usage.map(|u| AiUsage { prompt: u.prompt_tokens, completion: u.completion_tokens, total: u.total_tokens });
    Ok((content, usage))
}

/// 用自己的 key 生成。返回的形状和 `ai::ai_generate` 一样，前端可以共用同一套展示。
#[tauri::command]
pub async fn byok_generate(
    system_prompt: String,
    user_text: String,
    version: String,
    history: Option<Vec<ChatTurn>>,
) -> Result<AiResponse, ()> {
    let history = history.unwrap_or_default();
    let (content, usage) = match call_own_model(&system_prompt, &user_text, &history).await {
        Ok(pair) => pair,
        Err(e) => return Ok(failure(e)),
    };
    match remote::byok_build(&content, &version).await {
        Ok(resp) => Ok(AiResponse {
            ok: resp.ok,
            commands: resp.commands,
            loop_commands: resp.loop_commands,
            failures: resp.failures,
            explanation: resp.explanation,
            error: resp.error,
            // 用量以用户自己的上游为准，服务端构建这一步没有用量
            usage,
            // 多轮对话要把 AI 原话存进历史——用本地拿到的那份，服务端没回也不丢
            raw_content: resp.raw_content.or(Some(content)),
            balance: Some(resp.balance),
        }),
        Err(e) => {
            let mut f = failure(e);
            f.usage = usage;
            Ok(f)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_is_normalized_and_must_be_https_or_local() {
        assert_eq!(normalize_endpoint(" https://api.openai.com/v1/ ").unwrap(), "https://api.openai.com/v1");
        assert_eq!(
            normalize_endpoint("https://api.deepseek.com/v1/chat/completions").unwrap(),
            "https://api.deepseek.com/v1"
        );
        assert_eq!(normalize_endpoint("http://localhost:11434/v1").unwrap(), "http://localhost:11434/v1");
        assert!(normalize_endpoint("http://api.openai.com/v1").is_err(), "明文 http 发 key 不行");
        assert!(normalize_endpoint("http://localhost.evil.com/v1").is_err(), "不能用前缀骗过本机判断");
        assert!(normalize_endpoint("ftp://x").is_err());
        assert!(normalize_endpoint("https://").is_err());
    }

    #[test]
    fn key_format_is_checked_locally() {
        assert!(validate_key("sk-abcdefghijklmnop").is_ok());
        assert!(validate_key("").is_err());
        assert!(validate_key("sk-abc def ghi jkl").is_err());
        assert!(validate_key("sk-密钥密钥密钥密钥").is_err());
        assert!(validate_key("short").is_err());
    }

    #[test]
    fn key_hint_shows_last_four() {
        assert_eq!(key_hint("sk-abcdefgh1234"), "…1234");
    }

    #[test]
    fn plain_protection_round_trips() {
        #[cfg(not(windows))]
        {
            let (sealed, how) = protect("sk-test-key-123").unwrap();
            let st = Stored { key: sealed, protection: how.to_string(), ..Default::default() };
            assert_eq!(unprotect(&st).unwrap(), "sk-test-key-123");
        }
        let st = Stored { key: "abc".into(), protection: "dpapi-from-another-os".into(), ..Default::default() };
        assert!(unprotect(&st).is_err());
    }
}
