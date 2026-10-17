// Test-only worker boundary for deterministic scan and mutation ordering.
pub fn wait(variable: &str) {
    let Some(folder) = std::env::var_os(variable) else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    if !folder.is_dir() {
        return;
    }
    std::fs::write(folder.join("entered"), []).expect("worker gate marker");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !folder.join("release").exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
