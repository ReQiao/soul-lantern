//! 一键部署：扫描存档，把生成的命令打包成 datapack 写进存档。
//!
//! 存档从哪来（`datapack_list_saves`）：
//!   - 各平台默认的 .minecraft（Linux 另加 Flatpak 版的位置）；
//!   - 每个 .minecraft 都扫 `saves/` 和版本隔离用的 `versions/<实例>/saves/`
//!     （PCL2 / HMCL 等启动器开了版本隔离，存档就在后者）；
//!   - 用户手动选过的存档：记下来，下次连同它所在的 .minecraft 一起扫，
//!     所以用第三方启动器的人选一次，同一个目录下的其它存档和实例也都能扫到。
//! 扫到的、选过的存档和上次用的那个都存在配置目录的 `saves.json`，重启软件也还在。
//!
//! 为什么走 datapack 而不是替玩家把命令打进聊天栏：后者需要模拟输入或注入客户端，
//! 那是外挂的做法。datapack 是原版官方的内容分发方式——我们只是往存档目录写文件，
//! 玩家自己在游戏里执行 `/reload` 加载，全程没有任何非官方手段。
//!
//! 一次性命令 vs 循环命令：
//!   - commands（一次性）写进 soul_lantern:run，玩家自己执行一次
//!     `/function soul_lantern:run` 触发。
//!   - loop_commands（需要每 tick 侦测的，如箭矢/掉落物落地检测）写进 soul_lantern:tick，
//!     并通过 `data/minecraft/tags/function/tick.json` 挂到原版的 tick 函数标签上——
//!     `/reload` 之后自动每 tick 执行，不需要玩家再手动放一个循环命令方块。
//!     这是这次改造的关键点：凡是「持续侦测」的组合技，一键部署即可生效。
//!
//! 每次部署都是**整包替换**：先删掉整个 `PACK_DIR` 再重写。见 `datapack_deploy`。

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// datapack 的命名空间与函数名，最终对应游戏内的 `/function soul_lantern:run`。
///
/// 【定下来就别再改】玩家会把 `/function soul_lantern:run` 抄进命令方块、写进
/// 自己的其它 datapack。客户端没有自动更新，改一次名就是让所有人手里那些引用
/// 同时失效。以前叫 `soul`——太短太常见，和别的 datapack / 模组撞命名空间的概率
/// 不低，撞了之后谁覆盖谁取决于加载顺序，表现是"部署成功但函数是别人的"。
/// 趁还没正式发版改成和 `PACK_DIR` 同源的 `soul_lantern`。
const NAMESPACE: &str = "soul_lantern";
/// 改名之前用过的命名空间。部署时只要整包删掉重写，旧的自然就没了；
/// 单独留着这个常量是为了测试里能断言"旧命名空间确实被清干净"。
#[cfg(test)]
const LEGACY_NAMESPACE: &str = "soul";
const FUNCTION: &str = "run";
const TICK_FUNCTION: &str = "tick";
const PACK_DIR: &str = "soul_lantern_commands";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SaveInfo {
    pub name: String,
    pub path: String,
    /// 版本隔离的实例名（`versions/<实例>/saves/` 里的存档才有）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
}

#[derive(Serialize)]
pub struct SaveList {
    pub saves: Vec<SaveInfo>,
    /// 上次选的存档，前端打开时默认选它。
    pub last: Option<String>,
}

/// 记在 `saves.json` 里的东西。
#[derive(Default, Serialize, Deserialize)]
struct SavesMemory {
    #[serde(default)]
    saves: Vec<SaveInfo>,
    #[serde(default)]
    last: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployResult {
    /// datapack 写入的目录。
    pub pack_path: String,
    /// 一次性命令条数（写进 soul_lantern:run，需要玩家手动触发一次）。
    pub command_count: usize,
    /// 循环命令条数（写进 soul_lantern:tick 并挂 tick.json，`/reload` 后自动生效）。
    pub loop_command_count: usize,
    /// 玩家需要在游戏内执行的命令：/reload 必做；run_command 只有 command_count>0 时才需要，
    /// 循环部分 /reload 后自动生效，不需要玩家再做任何事。
    pub reload_command: String,
    pub run_command: Option<String>,
}

/// 各平台默认的 .minecraft，以及 Linux 上 Flatpak 版启动器的位置。
fn default_minecraft_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = minecraft_dir().into_iter().collect();
    #[cfg(target_os = "linux")]
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".var/app/com.mojang.Minecraft/.minecraft"));
    }
    dirs
}

/// 定位 .minecraft 目录（各平台默认位置）。
fn minecraft_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        // %APPDATA%\.minecraft
        dirs::config_dir()
            .map(|d| d.join(".minecraft"))
            .ok_or_else(|| "无法定位 %APPDATA% 目录".to_string())
    }
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .map(|d| d.join("Library/Application Support/minecraft"))
            .ok_or_else(|| "无法定位用户主目录".to_string())
    }
    #[cfg(target_os = "linux")]
    {
        dirs::home_dir()
            .map(|d| d.join(".minecraft"))
            .ok_or_else(|| "无法定位用户主目录".to_string())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("不支持的操作系统".to_string())
    }
}

/// 比较路径用的 key。Windows 的路径不分大小写，文件夹选择框和扫描出来的大小写可能不一样。
fn path_key(p: &str) -> String {
    let trimmed = p.trim_end_matches(['/', '\\']);
    if cfg!(windows) {
        trimmed.replace('/', "\\").to_lowercase()
    } else {
        trimmed.to_string()
    }
}

/// 列出一个 saves 目录下的存档。只认带 level.dat 的目录——saves 下常混有截图、
/// 备份之类的杂物，全列出来会让用户在下拉框里选到一个根本不是世界的目录。
fn scan_saves_dir(dir: &Path, instance: Option<&str>) -> Vec<SaveInfo> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .filter(|e| e.path().join("level.dat").is_file())
        .filter_map(|e| {
            Some(SaveInfo {
                name: e.file_name().to_str()?.to_string(),
                path: e.path().to_str()?.to_string(),
                instance: instance.map(str::to_string),
            })
        })
        .collect()
}

/// 一个 .minecraft 里的全部存档：`saves/` 加上每个 `versions/<实例>/saves/`。
fn scan_minecraft_dir(root: &Path) -> Vec<SaveInfo> {
    let mut out = scan_saves_dir(&root.join("saves"), None);
    if let Ok(versions) = fs::read_dir(root.join("versions")) {
        for v in versions.filter_map(Result::ok) {
            let Some(name) = v.file_name().to_str().map(str::to_string) else { continue };
            out.extend(scan_saves_dir(&v.path().join("saves"), Some(&name)));
        }
    }
    out
}

/// 从一个存档的路径反推它所在的 .minecraft 和实例名。
/// `X/saves/世界` → (X, None)；`X/versions/实例/saves/世界` → (X, Some(实例))。
/// 存档不在叫 saves 的文件夹里（用户把世界放在别处）就推不出来。
fn locate_save(save: &Path) -> Option<(PathBuf, Option<String>)> {
    let saves = save.parent()?;
    if saves.file_name()? != "saves" {
        return None;
    }
    let owner = saves.parent()?;
    let versions = owner.parent();
    if versions.and_then(Path::file_name).is_some_and(|n| n == "versions") {
        let instance = owner.file_name()?.to_str()?.to_string();
        Some((versions?.parent()?.to_path_buf(), Some(instance)))
    } else {
        Some((owner.to_path_buf(), None))
    }
}

fn save_info(path: &Path) -> Option<SaveInfo> {
    Some(SaveInfo {
        name: path.file_name()?.to_str()?.to_string(),
        path: path.to_str()?.to_string(),
        instance: locate_save(path).and_then(|(_, i)| i),
    })
}

/// 汇总所有能找到的存档：默认 .minecraft、记住的存档所在的 .minecraft、
/// 以及记住的存档本身（不在 saves 结构里的那种）。已经不存在的存档会被去掉。
fn collect_saves(default_roots: &[PathBuf], remembered: &[SaveInfo]) -> Vec<SaveInfo> {
    let mut roots: Vec<PathBuf> = default_roots.to_vec();
    for s in remembered {
        if let Some((root, _)) = locate_save(Path::new(&s.path)) {
            if !roots.iter().any(|r| path_key(&r.to_string_lossy()) == path_key(&root.to_string_lossy())) {
                roots.push(root);
            }
        }
    }

    let mut out: Vec<SaveInfo> = Vec::new();
    let push = |info: SaveInfo, out: &mut Vec<SaveInfo>| {
        if !out.iter().any(|s| path_key(&s.path) == path_key(&info.path)) {
            out.push(info);
        }
    };
    for root in &roots {
        for info in scan_minecraft_dir(root) {
            push(info, &mut out);
        }
    }
    for s in remembered {
        if Path::new(&s.path).join("level.dat").is_file() {
            push(s.clone(), &mut out);
        }
    }

    // 不隔离的存档排前面，然后按实例名、存档名
    out.sort_by(|a, b| (a.instance.is_some(), &a.instance, &a.name).cmp(&(b.instance.is_some(), &b.instance, &b.name)));
    out
}

fn memory_path() -> Option<PathBuf> {
    crate::paths::config_dir().map(|d| d.join("saves.json"))
}

fn load_memory() -> SavesMemory {
    memory_path()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// 先写临时文件再改名：手动模式和 AI 模式各有一个部署面板，可能同时在写。
fn store_memory(memory: &SavesMemory) {
    let Some(path) = memory_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let Ok(body) = serde_json::to_string_pretty(memory) else { return };
    let tmp = path.with_extension(format!("json.tmp{}", std::process::id()));
    if fs::write(&tmp, body).is_ok() {
        let _ = fs::rename(&tmp, &path);
    }
}

/// 扫描所有存档，并把结果记下来（下次打开软件直接能选）。
#[tauri::command]
pub fn datapack_list_saves() -> SaveList {
    let mut memory = load_memory();
    let saves = collect_saves(&default_minecraft_dirs(), &memory.saves);
    let last = memory
        .last
        .take()
        .filter(|l| saves.iter().any(|s| path_key(&s.path) == path_key(l)));
    memory.saves = saves.clone();
    memory.last = last.clone();
    store_memory(&memory);
    SaveList { saves, last }
}

/// 用户选了一个存档（手动浏览选的，或者在下拉框里换了一个）：记下来，并设为「上次用的」。
#[tauri::command]
pub fn datapack_remember_save(path: String) -> Result<SaveInfo, String> {
    let p: PathBuf = Path::new(&path).components().collect();
    if !p.join("level.dat").is_file() {
        return Err("这个文件夹不是存档（里面没有 level.dat）。请选 saves 文件夹里的某一个世界".to_string());
    }
    let info = save_info(&p).ok_or("存档路径里有无法识别的字符")?;
    let mut memory = load_memory();
    if let Some(existing) = memory.saves.iter().find(|s| path_key(&s.path) == path_key(&info.path)) {
        memory.last = Some(existing.path.clone());
        store_memory(&memory);
        return Ok(existing.clone());
    }
    memory.saves.push(info.clone());
    memory.last = Some(info.path.clone());
    store_memory(&memory);
    Ok(info)
}

/// 目标版本 → datapack 的 pack_format。
///
/// 取值来自各版本 server.jar 内 version.json 的 pack_version.data_major
/// （26.2 已实测为 107，见 scripts/mc-verifier/cache/26.2）。
/// 中间几个版本没有逐一实测，所以 pack.mcmeta 里额外写了 supported_formats 兜底，
/// 即便这里的数字偏差，datapack 仍能被游戏接受。
fn pack_format_for_version(version: &str) -> i32 {
    match version {
        "java_1_20_5" => 41,
        "java_1_21" | "java_1_21_1" => 48,
        "java_1_21_2" | "java_1_21_3" => 57,
        "java_1_21_4" => 61,
        "java_1_21_5" => 71,
        "java_1_21_6" => 80,
        "java_1_21_9" => 88,
        "java_1_21_11_plus" => 94,
        "java_26_1" => 101,
        "java_26_3_plus" => 121,
        _ => 107, // 26.2+（实测值）
    }
}

fn pack_metadata(version: &str) -> serde_json::Value {
    let format = pack_format_for_version(version);
    if format >= 82 {
        let minor = if matches!(version, "java_1_21_11_plus" | "java_26_1" | "java_26_2_plus") { 1 } else { 0 };
        serde_json::json!({"pack": {
            "pack_format": format,
            "description": "Soul Lantern 生成的指令",
            "min_format": [format, minor], "max_format": format
        }})
    } else {
        serde_json::json!({"pack": {
            "pack_format": format,
            "description": "Soul Lantern 生成的指令",
            "supported_formats": format
        }})
    }
}


/// 命令存进 .mcfunction 时不能带前导斜杠。空行与注释行原样保留。
fn clean_commands(commands: &[String]) -> Vec<String> {
    commands
        .iter()
        .map(|c| c.trim())
        .filter(|c| !c.is_empty())
        .map(|c| c.strip_prefix('/').unwrap_or(c).to_string())
        .collect()
}

/// 把命令写成 datapack 放进指定存档。
/// `commands` 是一次性命令（玩家手动触发一次）；`loop_commands` 是需要每 tick
/// 侦测的命令（自动挂 tick.json，`/reload` 后无需再做任何事）——两者可以只有一个非空。
#[tauri::command]
pub fn datapack_deploy(
    save_path: String,
    commands: Vec<String>,
    loop_commands: Vec<String>,
    version: String,
) -> Result<DeployResult, String> {
    let save = PathBuf::from(&save_path);
    if !save.join("level.dat").is_file() {
        return Err(format!("{save_path} 不像是一个存档目录（没有 level.dat）。"));
    }

    let body = clean_commands(&commands);
    let tick_body = clean_commands(&loop_commands);
    if body.is_empty() && tick_body.is_empty() {
        return Err("没有可部署的命令。".to_string());
    }

    let pack_dir = save.join("datapacks").join(PACK_DIR);

    // 【整包替换，先删后写】以前是逐个文件覆盖写、从不清理，于是：上一次部署带了
    // 循环命令、这一次没带，旧的 tick.mcfunction 和 tick.json 就一直留在存档里，
    // `/reload` 之后**继续每 tick 执行上一次的侦测逻辑**——用户以为换掉了，其实
    // 旧效果还在跑，而且界面上完全看不出来。命名空间改名之后，旧的 data/soul/
    // 也会同理残留。这个目录整个都是本工具生成的、名字也是我们独占的，直接删掉
    // 重建才符合"每次部署 = 这一次的内容"。
    if pack_dir.exists() {
        fs::remove_dir_all(&pack_dir).map_err(|e| format!("清理旧的 datapack 失败：{e}"))?;
    }

    // 1.21 起函数目录由 functions 改名为 function（单数）；两处都写，跨版本都能加载。
    let function_dirs = [
        pack_dir.join("data").join(NAMESPACE).join("function"),
        pack_dir.join("data").join(NAMESPACE).join("functions"),
    ];
    for dir in &function_dirs {
        fs::create_dir_all(dir).map_err(|e| format!("创建 datapack 目录失败：{e}"))?;
    }

    let meta = pack_metadata(&version);
    fs::write(pack_dir.join("pack.mcmeta"), serde_json::to_vec_pretty(&meta).unwrap_or_default())
        .map_err(|e| format!("写入 pack.mcmeta 失败：{e}"))?;

    let mut run_command = None;
    if !body.is_empty() {
        let content = format!("{}\n", body.join("\n"));
        for dir in &function_dirs {
            fs::write(dir.join(format!("{FUNCTION}.mcfunction")), &content)
                .map_err(|e| format!("写入 {FUNCTION}.mcfunction 失败：{e}"))?;
        }
        run_command = Some(format!("/function {NAMESPACE}:{FUNCTION}"));
    }

    if !tick_body.is_empty() {
        let content = format!("{}\n", tick_body.join("\n"));
        for dir in &function_dirs {
            fs::write(dir.join(format!("{TICK_FUNCTION}.mcfunction")), &content)
                .map_err(|e| format!("写入 {TICK_FUNCTION}.mcfunction 失败：{e}"))?;
        }
        // 1.21 起函数标签目录同样由 functions 改名为 function；两处都写。
        let tag_dirs = [
            pack_dir.join("data/minecraft/tags/function"),
            pack_dir.join("data/minecraft/tags/functions"),
        ];
        let tag = serde_json::json!({ "values": [format!("{NAMESPACE}:{TICK_FUNCTION}")] });
        let tag_bytes = serde_json::to_vec_pretty(&tag).unwrap_or_default();
        for dir in &tag_dirs {
            fs::create_dir_all(dir).map_err(|e| format!("创建 tick.json 目录失败：{e}"))?;
            fs::write(dir.join("tick.json"), &tag_bytes).map_err(|e| format!("写入 tick.json 失败：{e}"))?;
        }
    }

    Ok(DeployResult {
        pack_path: pack_dir.to_string_lossy().to_string(),
        command_count: body.len(),
        loop_command_count: tick_body.len(),
        reload_command: "/reload".to_string(),
        run_command,
    })
}

/// 猜测 .minecraft/saves 的默认位置，用于给「浏览选择存档」的文件夹选择框一个起始路径。
/// 猜不到（非官方启动器目录结构不同）或目录不存在时返回 None，前端会退回系统默认位置。
#[tauri::command]
pub fn datapack_default_saves_dir() -> Option<String> {
    let dir = minecraft_dir().ok()?.join("saves");
    dir.is_dir().then(|| dir.to_string_lossy().to_string())
}

/// 读 level.dat（gzip 压缩的 NBT）里 Data.Version.Name，识别存档的真实游戏版本。
///
/// 只返回原始版本字符串（如 "1.21.5"），不在这里做版本族分桶——那是 builder.ts
/// 里 VERSIONS 表已经维护的领域知识，重复一份容易和前端定义的分档边界慢慢对不上。
/// 读不到/解析失败（存档损坏、老版本 level.dat 结构不同）时返回 None，
/// 前端把它当作"识别不到，不打扰用户"处理，不阻塞部署流程。
#[tauri::command]
pub fn datapack_detect_version(save_path: String) -> Option<String> {
    let file = fs::File::open(PathBuf::from(save_path).join("level.dat")).ok()?;
    let mut reader = std::io::BufReader::new(file);
    let blob = nbt::Blob::from_gzip_reader(&mut reader).ok()?;
    let nbt::Value::Compound(data) = blob.get("Data")? else { return None };
    let nbt::Value::Compound(version) = data.get("Version")? else { return None };
    match version.get("Name")? {
        nbt::Value::String(name) => Some(name.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// 测试用的薄封装：省去每处都写 to_string 转换。一次性命令，无循环命令。
    fn deploy_for_test(save: &Path, commands: &[&str], version: &str) -> Result<DeployResult, String> {
        deploy_for_test_full(save, commands, &[], version)
    }

    fn deploy_for_test_full(
        save: &Path,
        commands: &[&str],
        loop_commands: &[&str],
        version: &str,
    ) -> Result<DeployResult, String> {
        datapack_deploy(
            save.to_string_lossy().to_string(),
            commands.iter().map(|s| s.to_string()).collect(),
            loop_commands.iter().map(|s| s.to_string()).collect(),
            version.to_string(),
        )
    }

    fn temp_save(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("soul-datapack-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("level.dat"), b"fake").unwrap();
        dir
    }

    /// 造一个结构和真实存档一致（Data.Version.Name）的 level.dat，供版本识别测试用。
    fn temp_save_with_version(name: &str, version_name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("soul-datapack-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mut version = nbt::Map::new();
        version.insert("Name".to_string(), nbt::Value::String(version_name.to_string()));
        let mut data = nbt::Map::new();
        data.insert("Version".to_string(), nbt::Value::Compound(version));
        let mut blob = nbt::Blob::new();
        blob.insert("Data", nbt::Value::Compound(data)).unwrap();

        let file = fs::File::create(dir.join("level.dat")).unwrap();
        let mut writer = std::io::BufWriter::new(file);
        blob.to_gzip_writer(&mut writer).unwrap();
        dir
    }

    #[test]
    fn writes_pack_and_function() {
        let save = temp_save("basic");
        let res = deploy_for_test(
            &save,
            &["/give @s minecraft:mace 1", "say hi"],
            "java_26_2_plus",
        )
        .unwrap();

        assert_eq!(res.command_count, 2);
        assert_eq!(res.loop_command_count, 0);
        assert_eq!(res.run_command, Some("/function soul_lantern:run".to_string()));

        let pack = save.join("datapacks").join(PACK_DIR);
        let meta: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(pack.join("pack.mcmeta")).unwrap()).unwrap();
        assert_eq!(meta["pack"]["pack_format"], 107);
        assert_eq!(meta["pack"]["min_format"], serde_json::json!([107, 1]));
        assert!(meta["pack"].get("supported_formats").is_none());

        // 前导斜杠必须去掉，否则游戏加载 function 时会报错
        let body = fs::read_to_string(pack.join("data/soul_lantern/function/run.mcfunction")).unwrap();
        assert_eq!(body, "give @s minecraft:mace 1\nsay hi\n");

        // 旧版单复数目录都要写
        assert!(pack.join("data/soul_lantern/functions/run.mcfunction").is_file());

        fs::remove_dir_all(&save).unwrap();
    }

    #[test]
    fn picks_pack_format_by_version() {
        assert_eq!(pack_format_for_version("java_1_20_5"), 41);
        assert_eq!(pack_format_for_version("java_1_21_4"), 61);
        assert_eq!(pack_format_for_version("java_26_2_plus"), 107);
        assert_eq!(pack_format_for_version("java_26_3_plus"), 121);
        assert_eq!(pack_metadata("java_26_3_plus")["pack"]["min_format"], serde_json::json!([121, 0]));
        assert_eq!(pack_metadata("java_1_20_5")["pack"]["supported_formats"], 41);
    }

    #[test]
    fn detects_real_version_from_level_dat() {
        let save = temp_save_with_version("detect-real", "1.21.5");
        assert_eq!(datapack_detect_version(save.to_string_lossy().to_string()), Some("1.21.5".to_string()));
    }

    #[test]
    fn detect_version_returns_none_for_garbage_level_dat() {
        // 已有的 temp_save 写的是假文件（不是合法 gzip NBT），不该 panic，应静默返回 None
        let save = temp_save("detect-garbage");
        assert_eq!(datapack_detect_version(save.to_string_lossy().to_string()), None);
    }

    #[test]
    fn detect_version_returns_none_for_missing_save() {
        assert_eq!(datapack_detect_version("/no/such/save/dir".to_string()), None);
    }

    #[test]
    fn rejects_non_save_dir() {
        let dir = std::env::temp_dir().join("soul-datapack-test-notasave");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let err = deploy_for_test(&dir, &["say hi"], "java_26_2_plus").unwrap_err();
        assert!(err.contains("level.dat"), "应提示这不是存档目录，实际：{err}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rejects_empty_commands() {
        let save = temp_save("empty");
        assert!(deploy_for_test(&save, &["", "   "], "java_26_2_plus").is_err());
        fs::remove_dir_all(&save).unwrap();
    }

    #[test]
    fn writes_tick_function_and_tag_for_loop_commands() {
        let save = temp_save("loop");
        let res = deploy_for_test_full(
            &save,
            &[],
            &[
                "execute at @e[type=minecraft:arrow,nbt={inGround:1b}] run summon minecraft:tnt ~ ~ ~ {fuse:0s}",
                "execute as @e[type=minecraft:arrow,nbt={inGround:1b}] run kill @s",
            ],
            "java_26_2_plus",
        )
        .unwrap();

        assert_eq!(res.command_count, 0);
        assert_eq!(res.loop_command_count, 2);
        // 没有一次性命令时不需要玩家手动触发任何东西
        assert_eq!(res.run_command, None);

        let pack = save.join("datapacks").join(PACK_DIR);
        let tick_body = fs::read_to_string(pack.join("data/soul_lantern/function/tick.mcfunction")).unwrap();
        assert!(tick_body.contains("summon minecraft:tnt"));
        assert!(tick_body.contains("run kill @s"));

        // tick.json 挂到原版的 tick 函数标签上，/reload 后自动每 tick 执行
        let tag: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(pack.join("data/minecraft/tags/function/tick.json")).unwrap())
                .unwrap();
        assert_eq!(tag["values"][0], "soul_lantern:tick");
        // 旧版单复数目录都要写
        assert!(pack.join("data/minecraft/tags/functions/tick.json").is_file());
        assert!(pack.join("data/soul_lantern/functions/tick.mcfunction").is_file());

        fs::remove_dir_all(&save).unwrap();
    }

    #[test]
    fn supports_both_once_and_loop_commands_together() {
        let save = temp_save("mixed");
        let res = deploy_for_test_full(&save, &["say hello"], &["say tick"], "java_26_2_plus").unwrap();
        assert_eq!(res.command_count, 1);
        assert_eq!(res.loop_command_count, 1);
        assert_eq!(res.run_command, Some("/function soul_lantern:run".to_string()));

        let pack = save.join("datapacks").join(PACK_DIR);
        assert!(pack.join("data/soul_lantern/function/run.mcfunction").is_file());
        assert!(pack.join("data/soul_lantern/function/tick.mcfunction").is_file());
        fs::remove_dir_all(&save).unwrap();
    }

    /// 上一次部署带循环命令、这一次不带：旧的 tick 必须消失。
    /// 以前是逐个文件覆盖写，旧的 tick.json 会留着，`/reload` 后继续每 tick 跑上一次的逻辑。
    #[test]
    fn redeploy_without_loop_removes_previous_tick() {
        let save = temp_save("redeploy-tick");
        deploy_for_test_full(&save, &["say once"], &["say every tick"], "java_26_2_plus").unwrap();
        let pack = save.join("datapacks").join(PACK_DIR);
        assert!(pack.join("data/minecraft/tags/function/tick.json").is_file());

        deploy_for_test(&save, &["say second"], "java_26_2_plus").unwrap();
        assert!(
            !pack.join("data/minecraft/tags/function/tick.json").exists(),
            "第二次部署没有循环命令，旧的 tick.json 必须被清掉"
        );
        assert!(!pack.join("data/minecraft/tags/functions/tick.json").exists());
        assert!(!pack.join("data/soul_lantern/function/tick.mcfunction").exists());
        let body = fs::read_to_string(pack.join("data/soul_lantern/function/run.mcfunction")).unwrap();
        assert_eq!(body, "say second\n");
        fs::remove_dir_all(&save).unwrap();
    }

    /// 改名前部署过的存档：旧命名空间 `soul` 的函数和 tick 标签不能残留。
    #[test]
    fn redeploy_cleans_up_legacy_namespace() {
        let save = temp_save("legacy-ns");
        let pack = save.join("datapacks").join(PACK_DIR);
        // 模拟老版本留下来的包
        let legacy = pack.join("data").join(LEGACY_NAMESPACE).join("function");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("tick.mcfunction"), "say old loop\n").unwrap();
        let tags = pack.join("data/minecraft/tags/function");
        fs::create_dir_all(&tags).unwrap();
        fs::write(tags.join("tick.json"), r#"{"values":["soul:tick"]}"#).unwrap();

        deploy_for_test(&save, &["say new"], "java_26_2_plus").unwrap();
        assert!(!pack.join("data").join(LEGACY_NAMESPACE).exists(), "旧命名空间目录必须清掉");
        assert!(!tags.join("tick.json").exists(), "旧的 tick 标签还挂着就会继续跑旧循环");
        assert!(pack.join("data/soul_lantern/function/run.mcfunction").is_file());
        fs::remove_dir_all(&save).unwrap();
    }

    /// 整包替换只能动我们自己的那个目录，同存档里玩家装的其它 datapack 不能被误删。
    #[test]
    fn redeploy_leaves_other_datapacks_alone() {
        let save = temp_save("other-packs");
        let other = save.join("datapacks").join("someone_elses_pack");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("pack.mcmeta"), "{}").unwrap();

        deploy_for_test(&save, &["say a"], "java_26_2_plus").unwrap();
        deploy_for_test(&save, &["say b"], "java_26_2_plus").unwrap();
        assert!(other.join("pack.mcmeta").is_file());
        fs::remove_dir_all(&save).unwrap();
    }

    #[test]
    fn rejects_when_both_command_lists_are_empty() {
        let save = temp_save("empty-both");
        assert!(deploy_for_test_full(&save, &[], &[], "java_26_2_plus").is_err());
        fs::remove_dir_all(&save).unwrap();
    }

    #[test]
    fn lists_only_real_saves() {
        let root = std::env::temp_dir().join("soul-datapack-test-mc");
        let _ = fs::remove_dir_all(&root);
        for w in ["saves/WorldB", "saves/WorldA", "versions/1.21.4/saves/Iso"] {
            fs::create_dir_all(root.join(w)).unwrap();
            fs::write(root.join(w).join("level.dat"), b"x").unwrap();
        }
        fs::create_dir_all(root.join("saves/screenshots")).unwrap(); // 杂物目录，无 level.dat
        fs::create_dir_all(root.join("versions/1.20")).unwrap(); // 没有 saves 的实例

        let saves = collect_saves(&[root.clone()], &[]);
        let got: Vec<(&str, Option<&str>)> = saves.iter().map(|s| (s.name.as_str(), s.instance.as_deref())).collect();
        assert_eq!(
            got,
            vec![("WorldA", None), ("WorldB", None), ("Iso", Some("1.21.4"))],
            "只列真存档；saves 和 versions/<实例>/saves 都扫；不隔离的排前面"
        );

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn remembered_save_brings_in_its_whole_minecraft_dir() {
        let base = std::env::temp_dir().join("soul-datapack-test-remember");
        let _ = fs::remove_dir_all(&base);
        let mc = base.join("PCL/.minecraft");
        for w in ["versions/PCL版/saves/Picked", "versions/PCL版/saves/Sibling", "saves/Plain"] {
            fs::create_dir_all(mc.join(w)).unwrap();
            fs::write(mc.join(w).join("level.dat"), b"x").unwrap();
        }
        // 不在 saves 结构里的存档，选过也要记住
        let loose = base.join("别处/MyWorld");
        fs::create_dir_all(&loose).unwrap();
        fs::write(loose.join("level.dat"), b"x").unwrap();

        let picked = save_info(&mc.join("versions/PCL版/saves/Picked")).unwrap();
        assert_eq!(picked.instance.as_deref(), Some("PCL版"));
        let loose_info = save_info(&loose).unwrap();
        let gone = SaveInfo { name: "Gone".into(), path: base.join("没了").to_string_lossy().into(), instance: None };

        let saves = collect_saves(&[], &[picked, loose_info, gone]);
        let mut names: Vec<&str> = saves.iter().map(|s| s.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["MyWorld", "Picked", "Plain", "Sibling"], "同一个 .minecraft 的其它存档也扫到，已删除的存档去掉");

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn locates_minecraft_dir_from_save_path() {
        let (root, inst) = locate_save(Path::new("/a/.minecraft/saves/W")).unwrap();
        assert_eq!((root, inst), (PathBuf::from("/a/.minecraft"), None));
        let (root, inst) = locate_save(Path::new("/a/.minecraft/versions/1.21/saves/W")).unwrap();
        assert_eq!((root, inst.as_deref()), (PathBuf::from("/a/.minecraft"), Some("1.21")));
        assert!(locate_save(Path::new("/a/somewhere/W")).is_none());
    }

    #[test]
    fn path_key_ignores_trailing_slash() {
        assert_eq!(path_key("/a/b/"), path_key("/a/b"));
    }
}
