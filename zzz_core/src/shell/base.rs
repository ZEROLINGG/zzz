use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use std::thread;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Windows 编码处理：使用 Win32 API 将本地编码转为 UTF-16 再转 String
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(windows)]
unsafe extern "system" {
    fn GetOEMCP() -> u32;
    fn GetACP() -> u32;
    fn MultiByteToWideChar(
        CodePage: u32,
        dwFlags: u32,
        lpMultiByteStr: *const u8,
        cbMultiByte: i32,
        lpWideCharStr: *mut u16,
        cchWideChar: i32,
    ) -> i32;
}

/// 使用指定代码页将字节流解码为 String
#[cfg(windows)]
fn decode_with_codepage(bytes: &[u8], code_page: u32) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    unsafe {
        // 第一次调用：获取转换后所需的 UTF-16 缓冲区长度
        let wide_len = MultiByteToWideChar(
            code_page,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            std::ptr::null_mut(),
            0,
        );

        if wide_len <= 0 {
            // 转换失败，退回 lossy UTF-8
            return String::from_utf8_lossy(bytes).to_string();
        }

        // 第二次调用：执行实际转换
        let mut wide_buf: Vec<u16> = vec![0u16; wide_len as usize];
        let written = MultiByteToWideChar(
            code_page,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            wide_buf.as_mut_ptr(),
            wide_len,
        );

        if written <= 0 {
            return String::from_utf8_lossy(bytes).to_string();
        }

        String::from_utf16_lossy(&wide_buf[..written as usize])
    }
}

/// Windows：优先尝试 UTF-8，失败后按系统代码页解码
#[cfg(windows)]
fn decode_output(bytes: &[u8], shell: &str) -> String {
    // ① 如果本身就是合法 UTF-8（例如已经 chcp 65001），直接返回
    if let Ok(s) = String::from_utf8(bytes.to_vec()) {
        return s;
    }

    // ② 根据 shell 类型选择代码页
    //    cmd.exe  → OEM Code Page（中文系统通常 = 936 / GBK）
    //    其它     → ANSI Code Page
    let code_page = unsafe {
        match shell {
            "cmd" => GetOEMCP(),
            _     => GetACP(),
        }
    };

    decode_with_codepage(bytes, code_page)
}

/// 非 Windows：直接 lossy UTF-8
#[cfg(not(windows))]
fn decode_output(bytes: &[u8], _shell: &str) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  主函数
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// 执行外部命令，支持指定 shell 和超时
pub fn exec(
    cmd: &str,
    shell: &str,        // cmd,powershell,sh,zsh,bash,python,python3....
    dir: Option<&str>,
    timeout_secs: Option<u32>,
) -> Result<(String, String), String> {
    let init_dir = match dir {
        Some(d) => d.to_string(),
        None => std::env::current_dir()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .to_string(),
    };

    let mut command = Command::new(shell);

    match shell {
        "cmd" => {
            command.arg("/C").arg(cmd);
        }
        "powershell" | "pwsh" => {
            command.arg("-Command").arg(cmd);
        }
        "sh" | "bash" | "zsh" => {
            command.arg("-c").arg(cmd);
        }
        "python" | "python3" => {
            command.arg("-c").arg(cmd);
        }
        _ => {
            return Err(format!("不支持的 shell: {}", shell));
        }
    }

    if let Some(d) = dir {
        command.current_dir(d);
    }

    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = command.spawn().map_err(|e| e.to_string())?;

    let timeout = timeout_secs.map(|s| Duration::from_secs(s as u64));
    let start = Instant::now();

    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(_status) => {
                let output = child.wait_with_output().map_err(|e| e.to_string())?;

                let stdout = decode_output(&output.stdout, shell);
                let stderr = decode_output(&output.stderr, shell);

                return if output.status.success() {
                    Ok((stdout, init_dir))
                } else {
                    Err(format!(
                        "命令执行失败\nstdout:\n{}\nstderr:\n{}",
                        stdout, stderr
                    ))
                };
            }
            None => {
                if let Some(t) = timeout {
                    if start.elapsed() >= t {
                        child.kill().map_err(|e| e.to_string())?;
                        return Err(format!("命令执行超时（{} 秒）", t.as_secs()));
                    }
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

#[test]
fn test_unix() {
    let r = exec("echo hello world", "sh", None, None);
    println!("{:?}", r);
}

