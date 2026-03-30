mod base;
pub mod full_shell;

pub fn exec(
    cmd: &str,
    shell: &str,        // cmd,powershell,sh,zsh,bash,python,python3....
    dir: Option<&str>,
    timeout_secs: Option<u32>,
) -> Result<(String, String), String>{
    base::exec(cmd, shell, dir, timeout_secs)
}
