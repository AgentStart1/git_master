# Git Master

A desktop application built with [GPUI](https://www.gpui.rs/) for viewing the status of every Git repository under a parent directory at a glance. Open a workspace directory to browse each repository's clean or dirty status, current branch, and ahead/behind counts in the left sidebar, and view repository details and commit history in the right panel.

## Features

- **Batch scanning**: Select a parent directory to automatically discover and alphabetically sort all Git repositories directly beneath it.
- **Status overview**: Each repository shows:
  - The current branch
  - Whether the working tree is clean (`✓` in green / `●` in red)
  - Its `↑ahead ↓behind` counts relative to the upstream branch
- **Details panel (Info tab)**: Displays the repository path, current branch, remote URL, and file status counts (added / modified / deleted / renamed / conflicted).
- **Commit history (Git Log tab)**: Displays the latest 200 commits, including the short hash, commit message, author, and timestamp.
- **Submodule support**: Repositories containing submodules can be expanded in the left sidebar. Select a submodule to view its details and commit history in the right panel.
- **Submodule initialization**: Uninitialized submodules display their status and URL and can be initialized from the right panel with `git submodule update --init`.
- **Non-blocking UI**: All Git I/O runs on background threads, keeping the interface responsive while repositories are scanned and details are loaded. Stale scan and detail results are discarded automatically.

## Technology Stack

- [`gpui`](https://crates.io/crates/gpui) — a GPU-accelerated Rust UI framework
- [`git2`](https://crates.io/crates/git2) — Rust bindings for libgit2
- [`chrono`](https://crates.io/crates/chrono) — commit timestamp formatting

## Build and Run

The Rust toolchain is required (edition 2024; a recent stable release is recommended).

```bash
# Run in development mode
cargo run

# Build a release binary
cargo build --release
./target/release/git_master
```

After launching the application, click **Open Directory** in the upper-right corner and select a parent directory containing multiple Git repositories.

## Project Structure

```
src/
├── main.rs              # Entry point and window creation
├── app_state.rs         # Application state and top-level Render implementation
├── git_ops.rs           # Repository scanning, details, and commit log retrieval via git2
├── models.rs            # Data structures such as RepoInfo, RepoDetail, and LogEntry
└── ui/
    ├── mod.rs
    ├── top_bar.rs       # Top directory selection bar
    ├── repo_list.rs     # Repository list in the left sidebar
    ├── detail_panel.rs  # Details and commit history panel on the right
    └── theme.rs         # Color constants
```

## Notes

- The application does not modify repositories except when the submodule initialization button in the right panel is used.
- Scanning only checks the direct children of the selected parent directory; it does not recursively search nested directories for repositories.
