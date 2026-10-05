mod downloads;
mod rcon;

use serde::Serialize;
use serde_json::json;
use std::{
    collections::VecDeque,
    fs,
    io::{BufRead, BufReader},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub type Result<T> = std::result::Result<T, String>;
pub type Progress = dyn Fn(&str) + Send + Sync;

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn check(&self) -> Result<()> {
        if self.0.load(Ordering::SeqCst) {
            Err("调试已关闭，操作已取消。".into())
        } else {
            Ok(())
        }
    }
}

// 权限跟随会话，不写进全局配置；换账号时必须取消仍在执行的旧任务。
#[derive(Default)]
pub struct SessionGate {
    owner: Option<String>,
    cancel: Cancellation,
}
impl SessionGate {
    pub fn enable(&mut self, token: String) -> Cancellation {
        self.disable();
        self.owner = Some(token);
        self.cancel.clone()
    }
    pub fn disable(&mut self) {
        self.cancel.cancel();
        self.owner = None;
        self.cancel = Cancellation::default();
    }
    pub fn permit(&self, token: &str) -> Result<Cancellation> {
        if self.owner.as_deref() == Some(token) {
            Ok(self.cancel.clone())
        } else {
            Err("请先在管理页开启当前账号的调试模式。".into())
        }
    }
}

pub struct Version {
    pub release: &'static str,
    java: u32,
    format: u32,
    minor: u32,
    plural: bool,
}
pub fn version(key: &str) -> Result<Version> {
    let (release, java, format, minor) = match key {
        "java_1_20_5" => ("1.20.5", 21, 41, 0),
        "java_1_21" => ("1.21", 21, 48, 0),
        "java_1_21_1" => ("1.21.1", 21, 48, 0),
        "java_1_21_2" => ("1.21.2", 21, 57, 0),
        "java_1_21_3" => ("1.21.3", 21, 57, 0),
        "java_1_21_4" => ("1.21.4", 21, 61, 0),
        "java_1_21_5" => ("1.21.5", 21, 71, 0),
        "java_1_21_6" => ("1.21.6", 21, 80, 0),
        "java_1_21_9" => ("1.21.9", 21, 88, 0),
        "java_1_21_11_plus" => ("1.21.11", 21, 94, 1),
        "java_26_1" => ("26.1", 25, 101, 1),
        "java_26_2_plus" => ("26.2", 25, 107, 1),
        "java_26_3_plus" => ("26.3", 25, 121, 0),
        "bedrock" => return Err("本机调试服务端只支持 Java 版。".into()),
        _ => return Err("不支持的调试版本。".into()),
    };
    Ok(Version {
        release,
        java,
        format,
        minor,
        plural: release == "1.20.5",
    })
}

#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Validation {
    pub version: String,
    pub status: String,
    pub message: String,
    pub response: String,
    pub logs: String,
}

// 单独保存子进程句柄，下载或 RCON 正在阻塞时，关闭调试仍能立即终止 Java。
#[derive(Default, Clone)]
pub struct ProcessControl(Arc<Mutex<Option<Child>>>);
impl ProcessControl {
    pub fn stop(&self) {
        if let Some(mut child) = self.0.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
    fn alive(&self) -> bool {
        let mut guard = self.0.lock().unwrap();
        guard
            .as_mut()
            .is_some_and(|c| matches!(c.try_wait(), Ok(None)))
    }
}

struct Server {
    version: String,
    work: tempfile::TempDir,
    port: u16,
    password: String,
    logs: Arc<Mutex<VecDeque<String>>>,
    control: ProcessControl,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.control.stop();
    }
}

pub struct Engine {
    cache: PathBuf,
    control: ProcessControl,
    server: Option<Server>,
}
impl Engine {
    pub fn new(cache: PathBuf, control: ProcessControl) -> Self {
        Self {
            cache,
            control,
            server: None,
        }
    }
    pub fn stop(&mut self) {
        self.server = None;
        self.control.stop();
    }

    pub fn validate(
        &mut self,
        key: &str,
        command: &str,
        cancel: &Cancellation,
        progress: &Progress,
    ) -> Result<Validation> {
        let result = self.validate_inner(key, command, cancel, progress);
        if result.is_err() {
            self.stop();
        }
        result
    }

    fn validate_inner(
        &mut self,
        key: &str,
        command: &str,
        cancel: &Cancellation,
        progress: &Progress,
    ) -> Result<Validation> {
        cancel.check()?;
        let v = version(key)?;
        let command = clean_command(command)?;
        if self
            .server
            .as_ref()
            .is_none_or(|s| s.version != v.release || !self.control.alive())
        {
            self.stop();
            fs::create_dir_all(&self.cache).map_err(|e| e.to_string())?;
            let jdks = downloads::jdks(&self.cache, cancel, progress)?;
            let java = &jdks[if v.java == 21 { 0 } else { 1 }];
            let jar = downloads::server_jar(&self.cache, v.release, cancel, progress)?;
            self.server = Some(start_server(
                &self.cache,
                &v,
                java,
                &jar,
                self.control.clone(),
                cancel,
                progress,
            )?);
        }
        cancel.check()?;
        let s = self.server.as_ref().unwrap();
        let mut rcon = rcon::Client::connect(s.port, &s.password)?;
        let pack = s.work.path().join("world/datapacks/soul_lantern_debug");
        let functions = pack.join("data/soul_lantern_debug").join(if v.plural {
            "functions"
        } else {
            "function"
        });
        fs::create_dir_all(&functions).map_err(|e| e.to_string())?;
        fs::write(pack.join("pack.mcmeta"), pack_metadata(&v).to_string())
            .map_err(|e| e.to_string())?;
        // 用函数加载验证完整语法，避开 RCON 对长指令的输入限制。
        for file in fs::read_dir(&functions)
            .map_err(|e| e.to_string())?
            .flatten()
        {
            if file.file_type().is_ok_and(|t| t.is_file()) {
                let _ = fs::remove_file(file.path());
            }
        }
        let id = format!("probe_{}", uuid::Uuid::new_v4().simple());
        fs::write(
            functions.join(format!("{id}.mcfunction")),
            format!("{command}\n"),
        )
        .map_err(|e| e.to_string())?;
        s.logs.lock().unwrap().clear();
        progress("正在加载并验证指令…");
        rcon.send("reload")?;
        // /reload 异步完成；等函数出现在注册表后才执行，失败也要等明确的加载日志。
        let deadline = Instant::now() + Duration::from_secs(30);
        let function = format!("soul_lantern_debug:{id}");
        let response = loop {
            cancel.check()?;
            if !self.control.alive() {
                return Err("调试服务端已退出。".into());
            }
            let tail = s
                .logs
                .lock()
                .unwrap()
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join("\n");
            if tail.contains(&format!("Failed to load function {function}")) {
                return Ok(Validation {
                    version: v.release.into(),
                    status: "syntax_error".into(),
                    message: "指令语法未通过官方服务端解析。".into(),
                    response: String::new(),
                    logs: tail,
                });
            }
            // reload 异步完成。不存在的函数不会执行；首次成功加载后只执行一次。
            let response = rcon.send(&format!("function {function}"))?;
            if response.contains("Executed ") && response.contains(&function) {
                break response;
            }
            let unknown = response.contains("Unknown function") && response.contains(&function);
            if !unknown {
                return Err(format!("无法判定调试函数的加载结果。\n{response}\n{tail}"));
            }
            if Instant::now() > deadline {
                return Err(format!("等待函数加载超时。\n{tail}"));
            }
            thread::sleep(Duration::from_millis(100));
        };
        thread::sleep(Duration::from_millis(100));
        cancel.check()?;
        let logs = s
            .logs
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        // 成功加载不等于实现用户意图；无人在线时 give/@s 等仍缺少执行上下文。
        Ok(Validation {
            version: v.release.into(),
            status: "syntax_ok".into(),
            message:
                "语法通过官方服务端解析；执行结果见下方。此世界没有在线玩家，目标与环境可能不满足。"
                    .into(),
            response,
            logs,
        })
    }
}

fn clean_command(command: &str) -> Result<&str> {
    let command = command.trim().trim_start_matches('/');
    if command.is_empty() || command.starts_with('#') || command.contains(['\n', '\r', '\0']) {
        return Err("每次验证一条完整指令，不能包含换行、注释或空内容。".into());
    }
    if command.len() > 1_000_000 {
        return Err("指令超过 1 MB，无法进行本机验证。".into());
    }
    Ok(command)
}

fn pack_metadata(v: &Version) -> serde_json::Value {
    if v.format >= 82 {
        json!({"pack":{"pack_format":v.format,"description":"Soul Lantern 本机调试",
            "min_format":[v.format,v.minor],"max_format":[v.format,v.minor]}})
    } else {
        json!({"pack":{"pack_format":v.format,"description":"Soul Lantern 本机调试","supported_formats":v.format}})
    }
}

fn free_port() -> Result<u16> {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .map_err(|e| e.to_string())
}
pub(crate) fn hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(not(windows))]
    let _ = command;
}

fn start_server(
    cache: &Path,
    v: &Version,
    java: &Path,
    jar: &Path,
    control: ProcessControl,
    cancel: &Cancellation,
    progress: &Progress,
) -> Result<Server> {
    cancel.check()?;
    progress(&format!(
        "正在启动 Minecraft {}（Java {}，最多 1 GB 堆内存）…",
        v.release, v.java
    ));
    let work = tempfile::Builder::new()
        .prefix("world-")
        .tempdir_in(cache)
        .map_err(|e| e.to_string())?;
    let port = free_port()?;
    let game_port = free_port()?;
    let password = uuid::Uuid::new_v4().simple().to_string();
    fs::write(work.path().join("eula.txt"), "eula=true\n").map_err(|e| e.to_string())?;
    fs::write(work.path().join("server.properties"), format!(
        "server-ip=127.0.0.1\nserver-port={game_port}\nonline-mode=false\nenable-rcon=true\nrcon.port={port}\nrcon.password={password}\nbroadcast-rcon-to-ops=false\nmax-players=0\nspawn-protection=0\nenable-command-block=false\nlevel-type=minecraft:normal\ngenerate-structures=false\nview-distance=2\nsimulation-distance=2\npause-when-empty-seconds=-1\nmax-tick-time=30000\nsync-chunk-writes=false\nfunction-permission-level=4\n"))
        .map_err(|e| e.to_string())?;
    let logs = Arc::new(Mutex::new(VecDeque::new()));
    let mut cmd = Command::new(java);
    hidden(&mut cmd);
    cmd.args([
        "-Xms256M",
        "-Xmx1024M",
        "-XX:ActiveProcessorCount=2",
        "-Dfile.encoding=UTF-8",
        "-jar",
    ])
    .arg(jar)
    .arg("nogui")
    .current_dir(work.path())
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("无法启动 Java：{e}"))?;
    for pipe in [
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn std::io::Read + Send>),
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn std::io::Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let logs = logs.clone();
        thread::spawn(move || {
            for line in BufReader::new(pipe)
                .lines()
                .map_while(std::result::Result::ok)
            {
                let mut tail = logs.lock().unwrap();
                tail.push_back(line.chars().take(4096).collect());
                if tail.len() > 160 {
                    tail.pop_front();
                }
            }
        });
    }
    {
        let mut slot = control.0.lock().unwrap();
        if let Err(e) = cancel.check() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }
        *slot = Some(child);
    }
    let server = Server {
        version: v.release.into(),
        work,
        port,
        password,
        logs,
        control,
    };
    let deadline = Instant::now() + Duration::from_secs(240);
    loop {
        cancel.check()?;
        let tail = server
            .logs
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        if !server.control.alive() {
            return Err(format!("Java 服务端启动失败。\n{tail}"));
        }
        if tail.contains("]: Done (")
            && rcon::Client::connect(server.port, &server.password).is_ok()
        {
            return Ok(server);
        }
        if Instant::now() > deadline {
            return Err(format!("服务端启动超时。\n{tail}"));
        }
        thread::sleep(Duration::from_millis(200));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn account_switch_revokes_old_jobs() {
        let mut gate = SessionGate::default();
        assert!(gate.permit("a").is_err());
        let a = gate.enable("a".into());
        assert!(gate.permit("b").is_err());
        let b = gate.enable("b".into());
        assert!(a.check().is_err());
        assert!(gate.permit("a").is_err());
        assert!(b.check().is_ok());
        gate.disable();
        assert!(b.check().is_err());
        assert!(gate.permit("b").is_err());
    }
    #[test]
    fn unsupported_versions_and_multiline_are_rejected() {
        for key in ["bedrock", "../../malicious", "1.20.4"] {
            assert!(version(key).is_err());
        }
        for c in ["", "say a\nsay b", "# comment", "say\0bad"] {
            assert!(clean_command(c).is_err());
        }
        assert_eq!(clean_command(" /give @s stone ").unwrap(), "give @s stone");
        assert_eq!(version("java_1_20_5").unwrap().java, 21);
        assert_eq!(version("java_26_3_plus").unwrap().java, 25);
    }
}
