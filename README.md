# Git Master

A desktop application built with [GPUI](https://www.gpui.rs/) for viewing the status of every Git repository under a parent directory at a glance. Open a workspace directory to browse each repository's clean or dirty status, current branch, and ahead/behind counts in the left sidebar, and view repository details and commit history in the right panel.

## Features

- **Batch scanning**: Select a parent directory to automatically discover and alphabetically sort all Git repositories directly beneath it.
- **Fetch All**: Fetch every configured remote of every project in the current repository list, pruning stale remote-tracking branches. Runs in the background, continues after failures, refreshes repository status and selected project details, and reports a summary with an operation log path on failure. Projects without remotes are skipped; submodules are not separately included in this batch.
- **Status overview**: Each repository shows:
  - The current branch
  - Whether the working tree is clean (`✓` in green / `●` in red)
  - Every remote's `↑ahead ↓behind` counts against the current branch (using the configured upstream branch name for its remote, otherwise the same branch name). `—` means no comparable fetched reference is available. Counts use locally fetched references; click **Fetch All** to update them.
- **Switch All to Main**: Switch every listed project to its default branch in the background. Uses `origin/HEAD`, then a default branch agreed on by other remotes, then local `main` or `master`. Creates a tracking branch if the identified remote default has no local branch. Never forces checkout or discards changes; failures are logged and other projects continue. Submodules are not separately switched.
- **Details panel (Info tab)**: Displays the repository path, current branch, remote URL, and file status counts (added / modified / deleted / renamed / conflicted).
- **Commit history (Git Log tab)**: Shared branch filters apply to both the list and canvas for the latest 200 commits. Switch branches and use Pull --rebase / Push on the current branch from the Git Log toolbar. Fetch and Reset remote controls are available in both views. The list draws parent links to show forks and merges; dashed tails indicate history outside the loaded range. The canvas supports draggable commit and HEAD cards, background panning, and wheel zooming of cards, text, and connections. HEAD cards sit beside their commits without a separate lane.
- **Sync colors**: Remote status is green when synchronized, blue when ahead, yellow when behind, red when diverged, and gray when comparison is unavailable.
- **Commit details**: Click a commit in List or Canvas (or its HEAD card) to open full hashes, parents, author/committer identities and dates, the complete message, and changed-file statistics below the history. Copy and Close controls are available. Submodule nodes use their own repository; dragging a canvas node does not open details. Merge changes are compared with the first parent.
- **Main/submodule commit graph**: The canvas places the main repository and its direct submodules in adjacent lanes. Dashed links show which submodule commit each main-repository commit records through its Git gitlink. References outside the loaded history, and references to uninitialized submodules, remain visible as placeholder nodes.
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
    ├── commit_canvas.rs # Commit graph layout and canvas interactions
    └── theme.rs         # Color constants
```

## Notes

- Reset requires a clean working tree and index, including no untracked files or in-progress Git operation. Commit or stash changes first. The check runs before and after Fetch; Reset still replaces local commits with the remote branch and requires confirmation.

- Right-click a project in the sidebar to open its directory or launch a terminal there (PowerShell on Windows, Terminal on macOS, `x-terminal-emulator` on Linux). Branch switching, Pull and Push are available in Git Log.

- Git action buttons modify repositories as indicated. Fetch All updates remote-tracking references without merging or changing working-tree files.
- Moving nodes only changes their visual position for the current application session; it does not reorder or modify Git history.
- Scanning only checks the direct children of the selected parent directory; it does not recursively search nested directories for repositories.
