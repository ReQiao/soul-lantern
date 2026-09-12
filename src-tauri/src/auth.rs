//! 账号相关的 Tauri 命令：前端 invoke 的入口，实际工作都转发给 remote.rs。
//!
//! 这一层刻意很薄，只做三件事：
//!   1. 把服务器发的 token 落到本地会话文件（前端拿不到 token 本身，见 session.rs）；
//!   2. 做一点点即时的格式校验，让明显打错的输入不用跑一趟网络就能得到反馈；
//!   3. 把服务端返回的中文错误原样透给前端——服务端的文案是精心措过辞的
//!      （比如"用户名或密码不对"对"账号不存在"和"密码错了"故意给同一句），
//!      这里不要自作主张改写，改写就可能把防枚举设计破坏掉。
//!
//! 真正的判断权全在服务器：本地校验通过不代表能注册成功，本地存着 token
//! 也不代表还登录着。

use crate::remote;
use crate::session;
use serde::Serialize;

/// 前端要展示的登录态。token 不在里面——那是故意的。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AuthState {
    pub logged_in: bool,
    pub username: String,
    pub phone_masked: String,
    pub balance: i64,
    pub activated: bool,
    /// 服务器连不上时为 true。界面要把"没登录"和"连不上服务器"分开说，
    /// 否则用户会以为自己账号出了问题，跑去反复重新注册。
    pub offline: bool,
    /// 这个账号是管理员。界面靠它在名字后面缀 `<管理员>`。
    pub is_admin: bool,
    /// 当前会话已经用 ADMIN_TOKEN 解锁过。管理页入口只在这个为 true 时出现。
    ///
    /// 和 `is_admin` 分开是服务端的设计：账号标记只管显示，真正的权限跟着
    /// 会话走、登出即失效、每次重新登录都要重新验一次 token。
    pub admin_verified: bool,
    /// 收藏的万灯集作品 id。
    pub favorites: Vec<String>,
    /// 管理员改过余额留给这个用户的通知。
    ///
    /// **服务端读到即清空**，所以这批数据一辈子只会到达客户端一次。
    /// 谁拿到谁就得负责弹出来——`auth_state` 每次调用都可能带回一批新的，
    /// 前端不能因为"这次不方便显示"就丢掉。
    pub notices: Vec<remote::BalanceNotice>,
}

impl AuthState {
    fn logged_out() -> Self {
        AuthState {
            logged_in: false,
            username: String::new(),
            phone_masked: String::new(),
            balance: 0,
            activated: false,
            offline: false,
            is_admin: false,
            admin_verified: false,
            favorites: Vec::new(),
            notices: Vec::new(),
        }
    }
}

/// 手机号本地预检。和服务端 crypto::normalize_phone 是同一套规则，两边各留一份
/// 是因为它们是两个独立部署的二进制，规则简单，保持一致比抽公共 crate 划算。
fn looks_like_phone(raw: &str) -> bool {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    let digits = digits.strip_prefix("86").unwrap_or(&digits);
    digits.len() == 11
        && digits.starts_with('1')
        && matches!(digits.as_bytes()[1], b'3'..=b'9')
}

/// 启动时问一次"我现在算登录着吗"。
///
/// 不看本地文件里的 expires_at 就直接下结论——本地时钟不准是常事，而且
/// 服务器可能因为改密码/管理员操作提前吊销了会话。以服务器的回答为准。
#[tauri::command]
pub async fn auth_state() -> Result<AuthState, ()> {
    if session::token().is_none() {
        return Ok(AuthState::logged_out());
    }
    match remote::me().await {
        Ok(me) => Ok(AuthState {
            logged_in: true,
            username: me.user.username,
            phone_masked: me.user.phone_masked,
            balance: me.balance,
            activated: me.activated,
            offline: false,
            is_admin: me.user.is_admin,
            admin_verified: me.admin_verified,
            favorites: me.user.favorites,
            notices: me.notices,
        }),
        Err(e) => {
            // remote::parse_json 遇到 401 已经清过本地会话了。这里只需要区分
            // "服务器说你没登录" 和 "根本没连上服务器"。
            let offline = e.starts_with("无法连接服务器");
            Ok(AuthState { offline, ..AuthState::logged_out() })
        }
    }
}

/// 服务端的登录门禁开关。取不到（服务器不可达）时默认**不要求**登录——
/// 宁可放行也不要把用户锁死在一个连不上服务器的门禁后面：反正真去调
/// /v1/ai/generate 还是会被服务端 401 挡住，安全性不受影响，
/// 但用户至少能看清"是服务器连不上"而不是对着一个打不开的登录框发呆。
#[tauri::command]
pub async fn auth_required() -> Result<bool, ()> {
    Ok(remote::server_version().await.map(|v| v.auth_required).unwrap_or(false))
}

/// 短信签名，给注册界面在**发码之前**提示用。
///
/// 拿不到就返回 None，界面退回泛化文案——连不上服务器时不该因为这个多弹一个错。
#[tauri::command]
pub async fn auth_sms_sign_name() -> Result<Option<String>, ()> {
    Ok(remote::server_version().await.ok().and_then(|v| v.sms_sign_name))
}

/// 服务端认为客户端太旧时给出的升级提示；不需要升级就是 None。
///
/// **说清楚它救不了谁**：这段代码是随新版客户端一起分发的，所以它对
/// "已经装着旧版、还没更新"的用户毫无作用——那批人只能靠服务端 401 文案
/// 自救（见 server/src/auth.rs 的 NEED_LOGIN_HINT）。这里接上是为了**下一次**
/// 不兼容变更时能提前告知，而不是为了这一次。
///
/// 之所以不上 tauri-plugin-updater：那要生成签名密钥对、配 pubkey/endpoints、
/// 开 createUpdaterArtifacts、还要维护一份 latest.json 清单。而服务端
/// `/v1/version` 的 minClient 字段本来就是为这件事留的，客户端也早就把它
/// 反序列化进来了，只差最后这段比较。
#[tauri::command]
pub async fn auth_upgrade_notice() -> Result<Option<String>, ()> {
    let Ok(v) = remote::server_version().await else { return Ok(None) };
    let current = env!("CARGO_PKG_VERSION");
    if !version_older_than(current, &v.min_client) {
        return Ok(None);
    }
    Ok(Some(format!(
        "当前版本 {current} 已经太旧，服务端要求至少 {}。\
         请到项目的 GitHub Releases 页面下载新版本，否则 AI 模式可能用不了。",
        v.min_client
    )))
}

/// 语义化版本比较，只比数字段。
///
/// 不引 semver crate：这里的版本号形态由发布脚本完全控制（那是作者本机的
/// 工具，不在这个仓库里），永远是 `x.y.0` 三段纯数字，没有预发布标签、
/// 没有 build metadata，
/// 为这点需求拉一个依赖不划算。段数不一样时缺的位当 0（`4.2` < `4.2.1`）。
fn version_older_than(current: &str, minimum: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> {
        s.split('.').map(|p| p.trim().parse().unwrap_or(0)).collect()
    };
    let (a, b) = (parse(current), parse(minimum));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x < y;
        }
    }
    false
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CodeSent {
    pub phone_masked: String,
    pub expires_in_secs: u32,
    pub log_mode: bool,
    /// 短信签名。界面要显示"短信开头写的是【XXX】"，让用户认得出这条码是我们发的。
    /// 不写死在前端——它是服务端 .env 里配的，以后换了前端写死就成了骗人。
    pub sign_name: Option<String>,
}

#[tauri::command]
pub async fn auth_register_begin(
    username: String,
    password: String,
    confirm_password: String,
    phone: String,
) -> Result<CodeSent, String> {
    // "两次密码不一致"完全可以在本地判断，没必要跑一趟网络
    if password != confirm_password {
        return Err("两次输入的密码不一样。".to_string());
    }
    if !looks_like_phone(&phone) {
        return Err("请输入 11 位中国大陆手机号。".to_string());
    }
    let r = remote::register_begin(username.trim(), &password, phone.trim()).await?;
    Ok(CodeSent {
        phone_masked: r.phone_masked,
        expires_in_secs: r.expires_in_secs,
        log_mode: r.log_mode,
        sign_name: r.sign_name,
    })
}

#[tauri::command]
pub async fn auth_register_resend(phone: String) -> Result<CodeSent, String> {
    let r = remote::register_resend(phone.trim()).await?;
    Ok(CodeSent {
        phone_masked: r.phone_masked,
        expires_in_secs: r.expires_in_secs,
        log_mode: r.log_mode,
        sign_name: r.sign_name,
    })
}

#[tauri::command]
pub async fn auth_register_verify(phone: String, code: String) -> Result<AuthState, String> {
    let s = remote::register_verify(phone.trim(), code.trim()).await?;
    session::save(&s.token, s.expires_at);
    Ok(AuthState {
        logged_in: true,
        username: s.user.username,
        phone_masked: s.user.phone_masked,
        balance: s.balance,
        activated: s.activated,
        offline: false,
        is_admin: s.user.is_admin,
        // 刚登录的会话一定没解锁管理权限——服务端每次发新会话都是 false，
        // 要用 ADMIN_TOKEN 单独解锁。
        admin_verified: s.admin_verified,
        favorites: s.user.favorites,
        // 登录响应不带通知。通知走 /v1/auth/me（auth_state），
        // 前端登录成功后本来就会刷一次登录态，那一次会拿到。
        notices: Vec::new(),
    })
}

#[tauri::command]
pub async fn auth_login(account: String, password: String) -> Result<AuthState, String> {
    let s = remote::login(account.trim(), &password).await?;
    session::save(&s.token, s.expires_at);
    Ok(AuthState {
        logged_in: true,
        username: s.user.username,
        phone_masked: s.user.phone_masked,
        balance: s.balance,
        activated: s.activated,
        offline: false,
        is_admin: s.user.is_admin,
        // 刚登录的会话一定没解锁管理权限——服务端每次发新会话都是 false，
        // 要用 ADMIN_TOKEN 单独解锁。
        admin_verified: s.admin_verified,
        favorites: s.user.favorites,
        // 登录响应不带通知。通知走 /v1/auth/me（auth_state），
        // 前端登录成功后本来就会刷一次登录态，那一次会拿到。
        notices: Vec::new(),
    })
}

#[tauri::command]
pub async fn auth_logout() -> Result<(), String> {
    // remote::logout 内部无论成败都会清本地会话
    let _ = remote::logout().await;
    Ok(())
}

#[tauri::command]
pub async fn auth_change_password(
    old_password: String,
    new_password: String,
    confirm_password: String,
) -> Result<(), String> {
    if new_password != confirm_password {
        return Err("两次输入的新密码不一样。".to_string());
    }
    remote::change_password(&old_password, &new_password).await
}

/// 改用户名。
///
/// 本地只做一点点即时校验（长度、「管理员」三个字），真正的判重和规范化在
/// 服务端——本地拦掉的是"不用跑一趟网络就知道不行"的那些。
#[tauri::command]
pub async fn auth_change_username(new_username: String) -> Result<AuthState, String> {
    let name = new_username.trim();
    let n = name.chars().count();
    if !(2..=24).contains(&n) {
        return Err("用户名需要 2~24 个字符。".to_string());
    }
    if name.contains("管理员") {
        return Err("用户名里不能含「管理员」三个字。".to_string());
    }
    remote::change_username(name).await?;
    auth_state().await.map_err(|_| "改完之后刷新登录态失败。".to_string())
}

/// 用 ADMIN_TOKEN 解锁当前会话的管理权限。
///
/// **token 不落盘**：只在这一次调用里出现，用完就没了。每次重新登录都要
/// 重新输——这是服务端"管理权限跟着会话走"设计的客户端一侧。
#[tauri::command]
pub async fn auth_admin_unlock(token: String) -> Result<AuthState, String> {
    let t = token.trim();
    if t.is_empty() {
        return Err("请输入管理员 token。".to_string());
    }
    remote::admin_unlock(t).await.map_err(|e| {
        // 服务端对"token 不对"和"这台服务器没配管理功能"故意返回同一个 404，
        // 这里也不要替它编一个更具体的解释。
        if e.contains("拒绝了这次请求") || e.contains("Not Found") {
            "管理员 token 不对，或者这台服务器没有开启管理功能。".to_string()
        } else {
            e
        }
    })?;
    auth_state().await.map_err(|_| "解锁之后刷新登录态失败。".to_string())
}

#[tauri::command]
pub async fn auth_reset_begin(phone: String) -> Result<CodeSent, String> {
    if !looks_like_phone(&phone) {
        return Err("请输入 11 位中国大陆手机号。".to_string());
    }
    let r = remote::reset_begin(phone.trim()).await?;
    Ok(CodeSent {
        phone_masked: r.phone_masked,
        expires_in_secs: r.expires_in_secs,
        log_mode: r.log_mode,
        sign_name: r.sign_name,
    })
}

#[tauri::command]
pub async fn auth_reset_confirm(
    phone: String,
    code: String,
    new_password: String,
    confirm_password: String,
) -> Result<(), String> {
    if new_password != confirm_password {
        return Err("两次输入的密码不一样。".to_string());
    }
    remote::reset_confirm(phone.trim(), code.trim(), &new_password).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison() {
        assert!(version_older_than("4.1.0", "4.2.0"));
        assert!(version_older_than("4.1.0", "5.0.0"));
        assert!(!version_older_than("4.2.0", "4.2.0"), "相等不算旧");
        assert!(!version_older_than("4.3.0", "4.2.0"));
        // 段数不一样时缺的位当 0
        assert!(version_older_than("4.2", "4.2.1"));
        assert!(!version_older_than("4.2.0", "4.2"));
        // 默认的 0.0.0 不该把任何版本判成过旧
        assert!(!version_older_than("4.1.0", "0.0.0"));
        // 多位数不能按字符串比（"10" > "9"）
        assert!(!version_older_than("4.10.0", "4.9.0"), "10 比 9 新，按字符串比会判反");
    }

    #[test]
    fn phone_precheck_matches_server_rules() {
        assert!(looks_like_phone("13800138000"));
        assert!(looks_like_phone("+86 138 0013 8000"));
        assert!(looks_like_phone("138-0013-8000"));
        assert!(!looks_like_phone("12800138000"), "第二位是 2 不是合法号段");
        assert!(!looks_like_phone("1380013800"), "10 位");
        assert!(!looks_like_phone(""));
        assert!(!looks_like_phone("abcdefghijk"));
    }
}
