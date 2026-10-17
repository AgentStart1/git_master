# Git Master

A desktop application built with [GPUI](https://www.gpui.rs/) for viewing the status of every Git repository under a parent directory at a glance. Open a workspace directory to browse each repository's clean or dirty status, current branch, and ahead/behind counts in the left sidebar, and view repository details and commit history in the right panel.

## Features

- **Batch scanning**: Select a parent directory to automatically discover and alphabetically sort all Git repositories directly beneath it.
- **Fetch All**: Fetch every configured remote of every project in the current repository list, pruning stale remote-tracking branches. Runs in the background, continues after failures, refreshes repository status and selected project details, and reports a summary with an operation log path on failure. Projects without remotes are skipped; submodules are not separately included in this batch.
- **Application logs**: Click Logs in the workspace header to open today's application and Git-operation log in a separate window. Logs refresh automatically every second and follow the latest entries. Scrolling up pauses following and keeps the displayed content and reading position stable while new entries are buffered. Scroll back to the bottom or click Follow latest to resume; Pause follow also pauses explicitly. Refresh, copy the displayed text, and scroll without leaving the current repository view. Repeated clicks activate the existing window. Large files show the latest 2,000 lines, up to 256 KiB; the full log stays on disk. Each refresh reopens the current daily file, including after rotation or midnight. If the latest entry alone exceeds the byte limit, its valid text suffix remains visible. Read failures keep the last readable content and reading position, including after a manual refresh; the status line shows the error while automatic retry continues.
- **Background task feedback**: A compact status bar shows the current task with an animated activity indicator. Results stay on one line; open Details for the complete output in a bounded, scrollable panel, or Copy to copy it. New task messages collapse previous details automatically.
- **Status overview**: Each repository shows:
  - The current branch
  - Whether the working tree is clean (`✓` in green / `●` in red)
  - Every remote's `↑ahead ↓behind` counts against the current branch (using the configured upstream branch name for its remote, otherwise the same branch name). `—` means no comparable fetched reference is available. Counts use locally fetched references; click **Fetch All** to update them.
- **Switch All to Main**: Switch every listed project to its default branch in the background. Uses `origin/HEAD`, then a default branch agreed on by other remotes, then local `main` or `master`. Creates a tracking branch if the identified remote default has no local branch. Never forces checkout or discards changes; failures are logged and other projects continue. Submodules are not separately switched.
- **Details panel (Info tab)**: Displays the repository path, current branch, remote URL, and file status counts (added / modified / deleted / renamed / conflicted).
- **Commit history (Git Log tab)**: Shared branch filters apply to both the list and canvas for the latest 200 commits. Switch branches or safely delete a non-current, fully merged local branch. Each remote provides Fetch, Pull --rebase, Push, and Reset controls; Pull uses Git's auto-stash behavior so local edits are restored after the rebase. The list draws parent links to show forks and merges; dashed tails indicate history outside the loaded range. The canvas supports draggable commit and HEAD cards, background panning, and wheel zooming of cards, text, and connections. HEAD cards sit beside their commits without a separate lane.
- **Git Change**: Lists changed and untracked files, previews staged and working-tree diffs, and supports Stage / Unstage per file without discarding working-tree edits.
- **Git Stash**: Saves tracked changes and untracked files, lists stashes, opens saved contents by clicking a stash name or View, shows commit information and file summaries with separate staged / working-tree / untracked diffs, and supports Apply, Pop, and confirmed deletion. Apply retains the stash; Pop removes it only after a successful application. Git identity must be configured to create a stash. Both tabs load on selection and provide Refresh; they also work for initialized submodules. Both diff views use a monospace code display, separate old/new line-number gutters, red/green removed/added rows, and word-level change highlights; empty sections are hidden. Difftastic is detected automatically on PATH and its structured output supplies line alignment and changed-token ranges. If it is missing or cannot render a preview, the built-in Git diff is used. Detection is repeated for each preview; no Git configuration changes are needed. Text previews are limited to 512 KiB per diff and code views to 2,000 rows.
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

For development setup, test commands, and the P0/P1 E2E coverage matrix, see [DEVELOPMENT.md](DEVELOPMENT.md).
