/**
 * 给人看的版本名。
 *
 * 内部版本号（package.json / Cargo.toml / tauri.conf.json 里的 `version`）必须是纯数字的
 * x.y.z：服务端按它做最低版本拦截（MIN_CLIENT），Windows 安装包也只认这种格式。
 * 所以「5.0-rc1」「5.0」这种名字单独放在这里，发版时和内部版本号一起改：
 *
 *   测试版：内部 5.0.0 → 显示 5.0-rc1
 *   正式版：内部 5.0.1 → 显示 5.0
 *
 * 安装包里的欢迎页文案在 src-tauri/installer/SimpChinese.nsh，发版时也要跟着改。
 */
export const DISPLAY_VERSION = "5.0-rc1";

/** 测试版（界面上会标「测试版」）。发正式版时改成 false。 */
export const IS_PRERELEASE = true;
