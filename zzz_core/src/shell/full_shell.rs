use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

enum ShellType {
    PowerShell,
    Cmd,
    Python,
    Bash,
    Unknown,
}

impl ShellType {
    fn from_path(path: &str) -> Self {
        let name = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        match name.as_str() {
            "powershell" | "pwsh" => ShellType::PowerShell,
            "cmd" => ShellType::Cmd,
            "python" | "python3" | "py" => ShellType::Python,
            "bash" | "sh" | "zsh" => ShellType::Bash,
            _ => ShellType::Unknown,
        }
    }
}

pub struct Shell {
    shell_path: String,
    child: Arc<Mutex<Option<Child>>>,
    stdin: Option<ChildStdin>,
    stdout_rx: Option<mpsc::Receiver<String>>,
    stderr_rx: Option<mpsc::Receiver<String>>,
    stdout_tx: Option<mpsc::Sender<String>>,
    stderr_tx: Option<mpsc::Sender<String>>,
    on_send_callback: Option<Arc<Mutex<Box<dyn FnMut(&str) -> Option<String> + Send>>>>,
}

impl Shell {
    pub fn new(shell: &str) -> Result<Self, String> {
        let (stdout_tx, stdout_rx) = mpsc::channel();
        let (stderr_tx, stderr_rx) = mpsc::channel();

        let (child, mut stdin) = Self::spawn_shell(shell)?;

        let stdout = child.lock()
            .map_err(|_| "mutex poisoned")?
            .as_mut()
            .and_then(|c| c.stdout.take())
            .ok_or("failed to capture stdout")?;

        let stderr = child.lock()
            .map_err(|_| "mutex poisoned")?
            .as_mut()
            .and_then(|c| c.stderr.take())
            .ok_or("failed to capture stderr")?;

        Self::spawn_reader(stdout, stdout_tx.clone());
        Self::spawn_reader(stderr, stderr_tx.clone());

        Self::init_shell_state(&mut stdin, shell)
            .map_err(|e| format!("failed to init UTF-8 encoding: {e}"))?;

        Ok(Self {
            shell_path: shell.to_string(),
            child,
            stdin: Some(stdin),
            stdout_rx: Some(stdout_rx),
            stderr_rx: Some(stderr_rx),
            stdout_tx: Some(stdout_tx),
            stderr_tx: Some(stderr_tx),
            on_send_callback: None,
        })
    }

    fn spawn_shell(shell: &str) -> Result<(Arc<Mutex<Option<Child>>>, ChildStdin), String> {
        let mut cmd = Command::new(shell);
        let shell_type = ShellType::from_path(shell);
        match shell_type {
            ShellType::PowerShell => {
                cmd.args(&[
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy", "Bypass",
                ]);
            }
            ShellType::Python => {
                cmd.args(&["-i", "-u"]);
            }
            _ => {}
        }

        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            unsafe {
                cmd.pre_exec(|| {
                    libc::setpgid(0, 0);
                    Ok(())
                });
            }
        }

        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to start shell: {e}"))?;

        let stdin = child.stdin.take().ok_or("failed to capture stdin")?;

        Ok((Arc::new(Mutex::new(Some(child))), stdin))
    }

    fn spawn_reader<R: Read + Send + 'static>(source: R, tx: mpsc::Sender<String>) {
        thread::spawn(move || {
            let reader = BufReader::new(source);
            for line in reader.lines().flatten() {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
    }

    fn init_shell_state(stdin: &mut ChildStdin, shell_path: &str) -> Result<(), std::io::Error> {
        let shell_type = ShellType::from_path(shell_path);

        match shell_type {
            ShellType::PowerShell => {
                stdin.write_all(
                    b"[Console]::InputEncoding  = [System.Text.Encoding]::UTF8\n\
                    [Console]::OutputEncoding = [System.Text.Encoding]::UTF8\n\
                    function prompt { '' }\n"
                )?;
            }
            ShellType::Cmd => {
                stdin.write_all(b"@echo off\r\nchcp 65001 >nul\r\n")?;
            }
            ShellType::Bash => {
                stdin.write_all(b"export LANG=en_US.UTF-8 LC_ALL=en_US.UTF-8\n")?;
            }
            ShellType::Python => {
                stdin.write_all(b"import sys; import os;import pathlib\n")?;
            }
            _ => {}
        }
        stdin.flush()?;
        Ok(())
    }


    /// 检查 stdin 流是否可用（未被关闭）
    fn check_stdin_open(&self) -> Result<(), std::io::Error> {
        if self.stdin.is_none() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "stdin is already closed — process may have exited or ^D was sent",
            ));
        }
        Ok(())
    }

    /// 检查子进程是否仍在运行
    /// - Ok(())  → 进程存活
    /// - Err(_)  → 进程已退出或 mutex 中毒
    fn check_process_alive(&self) -> Result<(), std::io::Error> {
        let mut guard = self.child.lock().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::Other, "child mutex poisoned")
        })?;

        match guard.as_mut() {
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "child process handle is gone",
            )),
            Some(child) => match child.try_wait() {
                Ok(None) => Ok(()), // 进程仍在运行
                Ok(Some(status)) => Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    format!(
                        "child process has already exited (exit code: {})",
                        status.code().unwrap_or(-1)
                    ),
                )),
                Err(e) => Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("failed to query child process status: {e}"),
                )),
            },
        }
    }

    /// 组合检查：同时验证 stdin 和进程状态
    /// 在任何写入操作前调用此方法
    fn check_ready_to_send(&self) -> Result<(), std::io::Error> {
        self.check_stdin_open()?;
        self.check_process_alive()?;
        Ok(())
    }

    // ──────────────── ^R 重置 Shell ────────────────

    pub fn reset(&mut self) -> Result<(), String> {
        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }

        let (new_child, mut new_stdin) = Self::spawn_shell(&self.shell_path)?;

        if let Some(ref tx) = self.stdout_tx {
            let stdout = new_child.lock()
                .map_err(|_| "mutex poisoned")?
                .as_mut()
                .and_then(|c| c.stdout.take())
                .ok_or("failed to capture stdout")?;
            Self::spawn_reader(stdout, tx.clone());
        }

        if let Some(ref tx) = self.stderr_tx {
            let stderr = new_child.lock()
                .map_err(|_| "mutex poisoned")?
                .as_mut()
                .and_then(|c| c.stderr.take())
                .ok_or("failed to capture stderr")?;
            Self::spawn_reader(stderr, tx.clone());
        }

        Self::init_shell_state(&mut new_stdin, &self.shell_path)
            .map_err(|e| format!("failed to init UTF-8 encoding: {e}"))?;

        self.child = new_child;
        self.stdin = Some(new_stdin);

        Ok(())
    }

    // ──────────────── 控制字符处理 ────────────────

    pub fn send_control_char(&mut self, c: &str) -> Result<(), std::io::Error> {
        let ch = c
            .chars()
            .next()
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "empty control sequence")
            })?
            .to_ascii_uppercase();

        match ch {
            // ^R 重置不需要检查（正是为了在进程挂死时恢复用的）
            'R' => {
                self.reset().map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::Other, e)
                })
            }
            'D' => {
                drop(self.stdin.take());
                Ok(())
            }
            _ => {
                self.check_ready_to_send()?;

                match ch {
                    'C' => {
                        #[cfg(unix)]
                        self.kill_pg(libc::SIGINT)?;
                        #[cfg(windows)]
                        self.write_raw_byte(0x03)?;
                        Ok(())
                    }
                    'Z' => {
                        #[cfg(unix)]
                        self.kill_pg(libc::SIGTSTP)?;
                        #[cfg(windows)]
                        {
                            drop(self.stdin.take());
                        }
                        Ok(())
                    }
                    _ => {
                        let byte = match ch {
                            '@' => 0x00u8,
                            'A'..='Z' => ch as u8 - b'A' + 1,
                            '?' => 0x7Fu8,
                            _ => {
                                return Err(std::io::Error::new(
                                    std::io::ErrorKind::InvalidInput,
                                    format!("unknown control character: ^{c}"),
                                ))
                            }
                        };
                        self.write_raw_byte(byte)
                    }
                }
            }
        }
    }

    fn write_raw_byte(&mut self, byte: u8) -> Result<(), std::io::Error> {
        let stdin = self.stdin.as_mut().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::BrokenPipe, "stdin already closed")
        })?;
        stdin.write_all(&[byte])?;
        stdin.flush()
    }

    #[cfg(unix)]
    fn kill_pg(&self, signal: i32) -> Result<(), std::io::Error> {
        let guard = self.child.lock().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::Other, "mutex poisoned")
        })?;
        if let Some(ref child) = *guard {
            let pid = child.id() as i32;
            let ret = unsafe { libc::kill(-pid, signal) };
            if ret != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        Ok(())
    }

    // ──────────────── send ────────────────

    pub fn send(&mut self, content: &str) -> Result<(), std::io::Error> {
        self.check_ready_to_send()?;

        let processed_content = if let Some(ref callback) = self.on_send_callback {
            let mut cb = callback.lock().map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::Other, "callback mutex poisoned")
            })?;

            match cb(content) {
                Some(processed) => processed,
                None => return Ok(()),
            }
        } else {
            content.to_string()
        };

        if processed_content.is_empty() {
            return Ok(());
        }

        if processed_content.len() < 5 {
            let trimmed = processed_content.trim();
            if trimmed.chars().count() == 2 {
                let mut chars = trimmed.chars();
                if let (Some('^'), Some(c)) = (chars.next(), chars.next()) {
                    if "CDZR".contains(c.to_ascii_uppercase()) {
                        // send_control_char 内部会按需进行二次检查
                        return self.send_control_char(&c.to_uppercase().to_string());
                    }
                }
            }
        }

        self.check_ready_to_send()?;

        let stdin = self.stdin.as_mut().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::BrokenPipe, "stdin already closed")
        })?;

        stdin.write_all(processed_content.as_bytes())?;
        stdin.flush()
    }

    // ──────────────── 回调注册 ────────────────

    pub fn on_send<F>(&mut self, func: F)
    where
        F: FnMut(&str) -> Option<String> + Send + 'static,
    {
        self.on_send_callback = Some(Arc::new(Mutex::new(Box::new(func))));
    }

    pub fn on_output<F>(&mut self, mut func: F)
    where
        F: FnMut(String) + Send + 'static,
    {
        if let Some(rx) = self.stdout_rx.take() {
            thread::spawn(move || {
                while let Ok(line) = rx.recv() {
                    func(line);
                }
            });
        }
    }

    pub fn on_error<F>(&mut self, mut func: F)
    where
        F: FnMut(String) + Send + 'static,
    {
        if let Some(rx) = self.stderr_rx.take() {
            thread::spawn(move || {
                while let Ok(line) = rx.recv() {
                    func(line);
                }
            });
        }
    }

    pub fn on_exit<F>(&self, mut func: F)
    where
        F: FnMut(i32) + Send + 'static,
    {
        let child = Arc::clone(&self.child);
        thread::spawn(move || {
            let code = loop {
                {
                    let mut guard = match child.lock() {
                        Ok(g) => g,
                        Err(_) => return,
                    };
                    match guard.as_mut() {
                        Some(c) => match c.try_wait() {
                            Ok(Some(status)) => break status.code().unwrap_or(-1),
                            Ok(None) => {}
                            Err(_) => break -1,
                        },
                        None => break -1,
                    }
                }
                thread::sleep(Duration::from_millis(200));
            };
            func(code);
        });
    }

    pub fn close(&mut self) {
        self.stdin.take();
        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        self.close();
    }
}