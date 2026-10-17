// Hold a completed scan on its worker so desktop cancellation tests can control
// publication order. This module is absent from production builds.
pub fn wait() {
    let Some(folder) = std::env::var_os("GIT_MASTER_TEST_SCAN_GATE") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::write(folder.join("entered"), []).expect("scan gate marker");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !folder.join("release").exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
