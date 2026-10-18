use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

// Hold an already-read snapshot on its worker, independently for each request.
// Desktop tests can then deliver an old window's result after its replacement.
pub fn wait() {
    let Some(folder) = std::env::var_os("GIT_MASTER_TEST_LOGS_RESULT_GATE") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    if !folder.join("armed").exists() {
        return;
    }
    let ticket = NEXT.fetch_add(1, Ordering::Relaxed);
    std::fs::write(folder.join(format!("{ticket}.entered")), []).expect("log result entered");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while folder.join(format!("{ticket}.hold")).exists()
        && !folder.join(format!("{ticket}.release")).exists()
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    std::fs::write(folder.join(format!("{ticket}.completed")), []).expect("log result completed");
}
