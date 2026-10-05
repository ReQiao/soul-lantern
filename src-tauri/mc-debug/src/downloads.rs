use crate::{Cancellation, Progress, Result, hidden};
use reqwest::blocking::Client;
use ring::digest::{Context, SHA1_FOR_LEGACY_USE_ONLY, SHA256};
use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

fn client() -> Result<Client> {
    Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .user_agent("Soul-Lantern-Minecraft-Debug/1")
        .build()
        .map_err(|e| e.to_string())
}
fn json(client: &Client, url: &str) -> Result<Value> {
    trusted_url(url)?;
    client
        .get(url)
        .timeout(Duration::from_secs(30))
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.json())
        .map_err(|e| format!("获取官方下载信息失败：{e}"))
}
fn trusted_url(url: &str) -> Result<()> {
    let url = reqwest::Url::parse(url).map_err(|e| e.to_string())?;
    if url.scheme() != "https"
        || !matches!(
            url.host_str(),
            Some(
                "api.azul.com"
                    | "cdn.azul.com"
                    | "piston-meta.mojang.com"
                    | "piston-data.mojang.com"
                    | "launchermeta.mojang.com"
                    | "launcher.mojang.com"
            )
        )
    {
        return Err("下载地址不是受支持的官方 HTTPS 来源。".into());
    }
    Ok(())
}
fn field<'a>(v: &'a Value, name: &str) -> Result<&'a str> {
    v[name]
        .as_str()
        .ok_or_else(|| format!("官方下载信息缺少 {name}。"))
}
fn hash_file(path: &Path, sha256: bool) -> Result<String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut ctx = Context::new(if sha256 {
        &SHA256
    } else {
        &SHA1_FOR_LEGACY_USE_ONLY
    });
    let mut buf = [0; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        ctx.update(&buf[..n]);
    }
    Ok(ctx
        .finish()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
fn download(
    client: &Client,
    url: &str,
    path: &Path,
    hash: &str,
    sha256: bool,
    label: &str,
    cancel: &Cancellation,
    progress: &Progress,
) -> Result<()> {
    trusted_url(url)?;
    let length = if sha256 { 64 } else { 40 };
    if hash.len() != length || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("官方校验值无效。".into());
    }
    if path.is_file() && hash_file(path, sha256)? == hash.to_ascii_lowercase() {
        return Ok(());
    }
    let parent = path.parent().ok_or("缓存目录无效。")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut last_error = String::new();
    for attempt in 1..=3 {
        cancel.check()?;
        progress(&format!("正在下载 {label}（第 {attempt} 次尝试）…"));
        let result = (|| {
            let mut response = client
                .get(url)
                .send()
                .and_then(|r| r.error_for_status())
                .map_err(|e| e.to_string())?;
            trusted_url(response.url().as_str())?;
            let total = response.content_length();
            let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
            let mut buf = [0; 64 * 1024];
            let mut count = 0u64;
            let mut last = Instant::now();
            loop {
                cancel.check()?;
                let n = response.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                count += n as u64;
                if count > 800 * 1024 * 1024 {
                    return Err("下载文件超过 800 MB，已停止。".into());
                }
                temp.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                if last.elapsed() > Duration::from_millis(500) {
                    progress(&format!(
                        "正在下载 {label}：{:.1} MB{}",
                        count as f64 / 1048576.,
                        total
                            .map(|t| format!(" / {:.1} MB", t as f64 / 1048576.))
                            .unwrap_or_default()
                    ));
                    last = Instant::now();
                }
            }
            temp.flush().map_err(|e| e.to_string())?;
            cancel.check()?;
            if hash_file(temp.path(), sha256)? != hash.to_ascii_lowercase() {
                return Err("下载文件校验失败。".into());
            }
            if path.exists() {
                fs::remove_file(path).map_err(|e| e.to_string())?;
            }
            temp.persist(path).map_err(|e| e.to_string())?;
            Ok(())
        })();
        match result {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_error = e;
                cancel.check()?;
            }
        }
    }
    Err(format!(
        "下载 {label} 失败：{last_error}。可稍后重试，已下载且校验通过的其他文件会保留。"
    ))
}

pub fn server_jar(
    cache: &Path,
    version: &str,
    cancel: &Cancellation,
    progress: &Progress,
) -> Result<PathBuf> {
    cancel.check()?;
    progress(&format!("正在查询 Minecraft {version} 官方服务端…"));
    let client = client()?;
    let manifest = json(
        &client,
        "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
    )?;
    let entry = manifest["versions"]
        .as_array()
        .and_then(|a| {
            a.iter()
                .find(|v| v["id"] == version && v["type"] == "release")
        })
        .ok_or("官方目录中没有这个正式版。")?;
    let meta = json(&client, field(entry, "url")?)?;
    let server = &meta["downloads"]["server"];
    let path = cache.join("servers").join(format!("{version}.jar"));
    download(
        &client,
        field(server, "url")?,
        &path,
        field(server, "sha1")?,
        false,
        &format!("Minecraft {version} 服务端"),
        cancel,
        progress,
    )?;
    Ok(path)
}

fn java_bin(root: &Path) -> PathBuf {
    root.join("bin")
        .join(if cfg!(windows) { "java.exe" } else { "java" })
}
fn is_jdk(root: &Path, major: u32) -> bool {
    let javac = root
        .join("bin")
        .join(if cfg!(windows) { "javac.exe" } else { "javac" });
    let mut cmd = Command::new(javac);
    hidden(&mut cmd);
    let Ok(out) = cmd.arg("-version").output() else {
        return false;
    };
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if !out.status.success()
        || !text
            .split_whitespace()
            .any(|s| s == major.to_string() || s.starts_with(&format!("{major}.")))
    {
        return false;
    }
    let mut cmd = Command::new(java_bin(root));
    hidden(&mut cmd);
    cmd.arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn installed(major: u32) -> Option<PathBuf> {
    for key in [
        format!("JAVA{major}_HOME"),
        format!("JAVA_HOME_{major}_X64"),
        "JAVA_HOME".into(),
    ] {
        if let Some(root) = std::env::var_os(key)
            .map(PathBuf::from)
            .filter(|p| is_jdk(p, major))
        {
            return Some(java_bin(&root));
        }
    }
    let mut cmd = Command::new("java");
    hidden(&mut cmd);
    if let Ok(out) = cmd.args(["-XshowSettings:properties", "-version"]).output() {
        let text = String::from_utf8_lossy(&out.stderr);
        if let Some(root) = text
            .lines()
            .find_map(|l| l.trim().strip_prefix("java.home = ").map(PathBuf::from))
            .filter(|p| is_jdk(p, major))
        {
            return Some(java_bin(&root));
        }
    }
    None
}

// 首次调试同时准备两代 JDK；只解压到自有缓存，不更改系统 Java 或 PATH。
pub fn jdks(cache: &Path, cancel: &Cancellation, progress: &Progress) -> Result<[PathBuf; 2]> {
    let mut found = Vec::new();
    for major in [21, 25] {
        cancel.check()?;
        found.push(if let Some(java) = installed(major) {
            java
        } else {
            download_jdk(cache, major, cancel, progress)?
        });
    }
    Ok([found.remove(0), found.remove(0)])
}

fn download_jdk(
    cache: &Path,
    major: u32,
    cancel: &Cancellation,
    progress: &Progress,
) -> Result<PathBuf> {
    let client = client()?;
    let target = cache.join(format!("jdk-{major}"));
    if is_jdk(&target, major) {
        return Ok(java_bin(&target));
    }
    let (os, extension) = match std::env::consts::OS {
        "windows" => ("windows", "zip"),
        "linux" => ("linux", "tar.gz"),
        "macos" => ("macos", "tar.gz"),
        _ => return Err("此平台不支持本机调试服务端。".into()),
    };
    let (arch, bitness) = match std::env::consts::ARCH {
        "x86_64" => ("x86", "64"),
        "aarch64" => ("arm", "64"),
        _ => return Err("本机调试需要 64 位 x86 或 ARM 系统。".into()),
    };
    progress(&format!("正在查询 JDK {major} 官方下载…"));
    let url = format!(
        "https://api.azul.com/metadata/v1/zulu/packages/?java_version={major}&os={os}&arch={arch}&hw_bitness={bitness}&java_package_type=jdk&javafx_bundled=false&release_status=ga&availability_types=CA&archive_type={extension}&latest=true&certifications=tck"
    );
    let packages = json(&client, &url)?;
    let package = packages
        .as_array()
        .and_then(|a| {
            a.iter().find(|p| {
                let name = p["name"].as_str().unwrap_or("");
                !name.contains("crac") && !name.contains("musl") && !name.contains("fx-")
            })
        })
        .ok_or("没有找到适合此系统的官方 JDK。")?;
    let detail = json(
        &client,
        &format!(
            "https://api.azul.com/metadata/v1/zulu/packages/{}",
            field(package, "package_uuid")?
        ),
    )?;
    let archive = cache.join(format!("jdk-{major}.{extension}"));
    download(
        &client,
        field(&detail, "download_url")?,
        &archive,
        field(&detail, "sha256_hash")?,
        true,
        &format!("JDK {major}"),
        cancel,
        progress,
    )?;
    progress(&format!("正在解压 JDK {major}…"));
    let staging = tempfile::Builder::new()
        .prefix("jdk-unpack-")
        .tempdir_in(cache)
        .map_err(|e| e.to_string())?;
    extract(&archive, staging.path(), os == "windows", cancel)?;
    let root = fs::read_dir(staging.path())
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|p| p.path())
        .find(|p| p.is_dir() && is_jdk(p, major))
        .ok_or("解压后的 JDK 不能运行。")?;
    cancel.check()?;
    if target.exists() {
        fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
    }
    fs::rename(root, &target).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(archive);
    Ok(java_bin(&target))
}

fn extract(archive: &Path, destination: &Path, zip: bool, cancel: &Cancellation) -> Result<()> {
    let file = fs::File::open(archive).map_err(|e| e.to_string())?;
    if zip {
        let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
        for i in 0..archive.len() {
            cancel.check()?;
            let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
            let relative = entry.enclosed_name().ok_or("压缩包路径不安全。")?;
            let path = destination.join(relative);
            if entry.is_dir() {
                fs::create_dir_all(path).map_err(|e| e.to_string())?;
            } else {
                fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
                let mut output = fs::File::create(path).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut output).map_err(|e| e.to_string())?;
            }
        }
    } else {
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
        for entry in archive.entries().map_err(|e| e.to_string())? {
            cancel.check()?;
            if !entry
                .map_err(|e| e.to_string())?
                .unpack_in(destination)
                .map_err(|e| e.to_string())?
            {
                return Err("压缩包路径不安全。".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn downloads_require_official_https() {
        for url in [
            "http://cdn.azul.com/file",
            "https://cdn.azul.com.evil.test/file",
            "file:///tmp/a",
        ] {
            assert!(trusted_url(url).is_err());
        }
        assert!(trusted_url("https://piston-data.mojang.com/a.jar").is_ok());
    }
    #[test]
    fn hash_mismatch_never_uses_cached_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.jar");
        fs::write(&path, b"abc").unwrap();
        assert_eq!(
            hash_file(&path, true).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_ne!(hash_file(&path, false).unwrap(), "0".repeat(40));
    }
    #[test]
    #[ignore = "仅在云端验证首次使用的官方 JDK 下载与解压"]
    fn first_use_downloads_both_official_jdks() {
        let cache = tempfile::tempdir().unwrap();
        let cancel = Cancellation::default();
        for major in [21, 25] {
            let java =
                download_jdk(cache.path(), major, &cancel, &|text| println!("{text}")).unwrap();
            assert!(java.is_file());
            assert!(is_jdk(&cache.path().join(format!("jdk-{major}")), major));
            let cached = download_jdk(cache.path(), major, &cancel, &|_| {
                panic!("有效 JDK 缓存不应重新下载")
            })
            .unwrap();
            assert_eq!(java, cached);
        }
    }
}
