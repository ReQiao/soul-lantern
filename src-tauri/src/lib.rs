// pub mod（不是 mod）是为了 tests/ 下的集成测试能直接调用这些模块里的
// tauri::command 函数，用真实网络请求走一遍完整链路，而不是只测内部逻辑。
pub mod admin;
pub mod ai;
pub mod auth;
pub mod billing;
mod datapack;
pub mod plaza;
pub mod remote;
pub mod session;
pub mod window;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            ai::ai_generate,
            auth::auth_state,
            auth::auth_required,
            auth::auth_sms_sign_name,
            auth::auth_upgrade_notice,
            auth::auth_register_begin,
            auth::auth_register_resend,
            auth::auth_register_verify,
            auth::auth_login,
            auth::auth_logout,
            auth::auth_change_password,
            auth::auth_change_username,
            auth::auth_admin_unlock,
            auth::auth_reset_begin,
            auth::auth_reset_confirm,
            billing::billing_state,
            billing::billing_activate,
            billing::billing_topup_tiers,
            billing::billing_recharge,
            datapack::datapack_list_saves,
            datapack::datapack_deploy,
            datapack::datapack_default_saves_dir,
            datapack::datapack_detect_version,
            admin::admin_users,
            admin::admin_lookup,
            admin::admin_adjust_balance,
            admin::admin_delete_user,
            admin::admin_get_env,
            admin::admin_set_env,
            admin::admin_get_policy,
            admin::admin_set_policy,
            admin::admin_impersonate,
            admin::admin_health,
            admin::admin_restart,
            plaza::plaza_categories,
            plaza::plaza_list,
            plaza::plaza_get,
            plaza::plaza_publish,
            plaza::plaza_delete,
            plaza::plaza_like,
            plaza::plaza_favorite,
            plaza::plaza_download,
            plaza::plaza_comment,
            plaza::plaza_delete_comment,
            window::window_toggle_fullscreen,
            window::window_is_fullscreen,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
