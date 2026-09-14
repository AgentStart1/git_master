use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

static LOG_LOCK: Mutex<()> = Mutex::new(());

pub fn log_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_STATE_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("GitMaster").join("logs").join(format!(
        "operations-{}.log",
        chrono::Local::now().format("%Y-%m-%d")
    ))
}

pub fn append(message: &str) -> std::io::Result<()> {
    let _guard = LOG_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let path = log_path();
    std::fs::create_dir_all(path.parent().expect("log directory"))?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(
        file,
        "{} pid={} {}",
        chrono::Local::now().to_rfc3339(),
        std::process::id(),
        redact(message)
    )?;
    file.flush()
}

// Remove HTTP URL credentials and query parameters from Git's diagnostic output.
fn redact(message: &str) -> String {
    let mut output = String::new();
    for token in message.split_inclusive(char::is_whitespace) {
        if let Some(start) = token.find("://") {
            let prefix = &token[..start + 3];
            let rest = &token[start + 3..];
            let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
            let rest = if let Some(at) = rest[..authority_end].rfind('@') {
                format!("[redacted]@{}", &rest[at + 1..])
            } else {
                rest.to_string()
            };
            output.push_str(prefix);
            if let Some(query) = rest.find(['?', '#']) {
                output.push_str(&rest[..query]);
                output.push_str("[redacted]");
                output.push_str(&token[token.trim_end().len()..]);
            } else {
                output.push_str(&rest);
            }
        } else {
            output.push_str(token);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    #[test]
    fn url_secrets_are_removed_without_losing_error_context() {
        assert_eq!(
            super::redact("fatal: https://user:secret@host/repo?token=abc\nfailed"),
            "fatal: https://[redacted]@host/repo[redacted]\nfailed"
        );
    }
}
