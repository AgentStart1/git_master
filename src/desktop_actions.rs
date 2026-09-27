use std::{path::Path, process::Command};

fn command(path: &Path, terminal: bool) -> Command {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        if terminal {
            let mut command = Command::new("powershell.exe");
            command
                .args(["-NoLogo", "-NoProfile", "-NoExit"])
                .current_dir(path)
                .creation_flags(0x00000010); // Interactive new console.
            command
        } else {
            let mut command = Command::new("explorer.exe");
            command.arg(path);
            command
        }
    }
    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        if terminal {
            command.args(["-a", "Terminal"]);
        }
        command.arg(path);
        command
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let mut command = Command::new(if terminal {
            "x-terminal-emulator"
        } else {
            "xdg-open"
        });
        if terminal {
            command.current_dir(path);
        } else {
            command.arg(path);
        }
        command
    }
}

pub fn open(path: &Path, terminal: bool) -> Result<(), String> {
    let path = path.canonicalize().map_err(|error| error.to_string())?;
    if !path.is_dir() {
        return Err("Directory is unavailable".into());
    }
    // Explorer and PowerShell expect ordinary Windows paths, not verbatim paths.
    #[cfg(target_os = "windows")]
    let path = {
        let value = path.to_string_lossy();
        std::path::PathBuf::from(if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{unc}")
        } else {
            value.strip_prefix(r"\\?\").unwrap_or(&value).to_owned()
        })
    };
    let mut child = command(&path, terminal)
        .spawn()
        .map_err(|error| error.to_string())?;
    // Reap launcher processes without blocking the UI or waiting for terminal closure.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    #[test]
    fn paths_are_arguments_or_working_directories_never_shell_code() {
        let path = Path::new(r"C:\repos\project & other");
        let directory = command(path, false);
        assert_eq!(directory.get_args().collect::<Vec<_>>(), [path.as_os_str()]);
        let terminal = command(path, true);
        assert_eq!(terminal.get_current_dir(), Some(path));
        assert_eq!(
            terminal.get_args().collect::<Vec<_>>(),
            ["-NoLogo", "-NoProfile", "-NoExit"]
        );
    }
}
