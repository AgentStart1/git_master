// A native subprocess double: detection succeeds, rendering returns malformed JSON.
fn main() {
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("Difftastic test fixture");
        return;
    }
    use std::io::Write;
    let path = std::env::var_os("GIT_MASTER_TEST_DIFFT_CALLS").unwrap();
    let mut calls = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    writeln!(calls, "render").unwrap();
    println!("this is not valid JSON");
}
