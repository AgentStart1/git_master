# Git Master

A desktop application built with [GPUI](https://www.gpui.rs/) for viewing the status of every Git repository under a parent directory at a glance. Open a workspace directory to browse each repository's clean or dirty status, current branch, and ahead/behind counts in the left sidebar, and view repository details and commit history in the right panel.

## Features

- **Workspace navigation**: The workspace menu remembers the last eight directories locally. Search repositories by name or path and filter All / Modified / To sync. Repositories are grouped into Needs attention and Other repositories; filtering preserves the selected repository. Expanded submodules remain available beneath their parent.
- **Appearance**: Light / Dark controls in the workspace header update the main window, commit canvas, diff previews and open Logs window together. The choice is remembered locally across restarts. Switching themes preserves navigation, branch filters, selected commit and scroll state. Button focus outlines appear during keyboard navigation; mouse clicks keep normal pressed/selected feedback without leaving a focus border.
- **Branch controls**: Tab/arrow keys navigate the branch menu, Space/Enter activates the focused branch, and Escape closes it. The repository header shows the current branch and a separate Switch branch menu for checkout and safe deletion. In History, Show branches opens a searchable multi-select menu shared by List and Canvas. Select all or Current only changes the display without checking out a branch; at least one branch remains visible.

- **Batch scanning**: Select a parent directory to automatically discover and alphabetically sort all Git repositories directly beneath it.
- **Local refresh**: Reopening the current workspace refreshes the repository list and selected repository details. Every 30 seconds, idle workspaces also refresh local status, history and the visible changes or stash preview; this does not fetch from remotes. Automatic refresh skips active operations, reads and open menus/dialogs, and preserves repository selection, branch filters and the loaded history range.
- **Fetch All**: Fetch every configured remote of every project in the current repository list, pruning stale remote-tracking branches. Runs in the background, continues after failures, refreshes repository status and selected project details, and shows per-item success, failure and skipped results. Retry one failed item or all failed items without rerunning successful remotes. Projects without remotes are skipped; submodules are not separately included in this batch.
- **Application logs**: Click Logs in the workspace header to open today's application and Git-operation log in a separate window. Logs refresh automatically every second and follow the latest entries. Scrolling up pauses following and keeps the displayed content and reading position stable while new entries are buffered. Scroll back to the bottom or click Follow latest to resume; Pause follow also pauses explicitly. Resizing keeps the latest line visible while following and retains your place in wrapped text while paused. Choose a saved log file, return to today’s log, open the log directory, refresh, or copy the displayed text, and scroll without leaving the current repository view. Repeated clicks activate the existing window. Large files show the latest 2,000 lines, up to 256 KiB; the full log stays on disk. Each refresh reopens the current daily file, including after rotation or midnight. If the latest entry alone exceeds the byte limit, its valid text suffix remains visible. Read failures keep the last readable content and reading position, including after a manual refresh; the status line shows the error while automatic retry continues.
- **Background task feedback**: A compact status bar stays at the bottom of the window and shows the current task with an animated activity indicator. Results stay on one line; open Details to expand complete output or batch results above the bar in a bounded, scrollable panel, or Copy to copy it. New task messages collapse previous details automatically.
- **Status overview**: Each repository shows:
  - The current branch
  - Whether the working tree is clean (`✓` in green / `●` in red)
  - Every remote's `↑ahead ↓behind` counts against the current branch (using the configured upstream branch name for its remote, otherwise the same branch name). `—` means no comparable fetched reference is available. Counts use locally fetched references; click **Fetch All** to update them.
- **Switch all to default**: Switch every listed project to its default branch in the background. Uses `origin/HEAD`, then a default branch agreed on by other remotes, then local `main` or `master`. Creates a tracking branch if the identified remote default has no local branch. Never forces checkout or discards changes; failures are logged and other projects continue. Submodules are not separately switched.
- **Details panel (Overview tab)**: Displays the repository path, current branch, remote URL, and file status counts (added / modified / deleted / renamed / conflicted).
- **Commit history (History tab)**: Shared branch filters apply to both the list and canvas. History initially includes up to 200 commits per selected branch and matching remote branch, so older branch tips remain visible. Use **Load 200 more per branch** to continue into older history; the canvas also loads another batch when you pan near the bottom. Existing cards, dragged positions, pan, and zoom stay fixed while history is appended. Switch branches or safely delete a non-current, fully merged local branch. Each remote provides Fetch, Pull --rebase, Push, and Reset controls; Pull uses Git's auto-stash behavior so local edits are restored after the rebase. The list draws parent links to show forks and merges; dashed tails indicate history outside the loaded range. The canvas uses the rounded commit cards themselves as nodes, without external dots, ports or a side timeline. Parent edges join card bottoms to parent-card tops; gray dashed gitlinks join the main commit card’s right edge to the module card’s left edge. Routes avoid card content and nearby HEAD reference labels. Blue dashed parent edges mark unloaded history. Cards and reference labels can be dragged; background panning and wheel zoom preserve the graph.
- **Changes**: Lists changed and untracked files, previews staged and working-tree diffs, and supports Stage / Unstage per file without discarding working-tree edits.
- **Stashes**: Saves tracked changes and untracked files, lists stashes, opens saved contents by clicking a stash name or View, shows commit information and file summaries with separate staged / working-tree / untracked diffs, and supports Apply, Pop, and confirmed deletion. Apply retains the stash; Pop removes it only after a successful application. Git identity must be configured to create a stash. Both tabs load on selection and provide Refresh; they also work for initialized submodules. Both diff views use a monospace code display, separate old/new line-number gutters, red/green removed/added rows, and word-level change highlights; empty sections are hidden. Difftastic is detected automatically on PATH and its structured output supplies line alignment and changed-token ranges. If it is missing or cannot render a preview, the built-in Git diff is used. Detection is repeated for each preview; no Git configuration changes are needed. Text previews are limited to 512 KiB per diff and code views to 2,000 rows.
- **Sync colors**: Remote status is green when synchronized, blue when ahead, yellow when behind, red when diverged, and gray when comparison is unavailable.
- **Commit details**: Click a commit in List or Canvas (or its HEAD reference label) to open full hashes, parents, author/committer identities and dates, the complete message, and changed-file statistics in a right-hand inspector beside the history. Message encoding is decoded by Git into UTF-8 for both display and copying. Commit bodies support Markdown headings, emphasis, lists, task lists, quotes, code blocks, tables and web/mail links, with Preview / Source switching and original-body copy. HTML stays literal and images show alt text without loading external resources. Copy and Close controls are available, with inline hash copy and expandable read-only file differences. Submodule nodes use their own repository; dragging a canvas node does not open details. Merge changes are compared with the first parent.
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

After launching the application, open the workspace menu beside **Git Master**, choose **打开工作区** and select a parent directory containing multiple Git repositories.

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

## Views

- **Changes** groups staged, unstaged, untracked and conflicted files. The preview compares HEAD/index or index/working tree for the selected group; Stage and Unstage preserve working files. Conflicts show a bounded, read-only current file plus independent collapsible index-stage views and external recovery steps. Marking a conflict resolved requires confirmation before staging; it does not merge or validate the resolution automatically.
- **Stashes** separates saved index, working-tree and untracked content. Apply retains the record; Pop removes it only after successful application with a conflict-free index. Conflicts retain the record and show recovery guidance. Deletion uses a themed confirmation with Cancel focused initially.
- **Overview** shows repository path, branch, HEAD, Git state, changes, remote comparisons and submodules, with copy/open/fetch/navigation actions. Remote addresses are redacted. Comparison selection does not change Git upstream configuration.
- **Canvas** displays local and fetched remote branch-tip reference cards, including symbolic remote HEAD when available. Thin reference lines connect each label card directly to its commit and follow dragging; they are distinct from commit-parent and submodule links. It supports zoom, fit, return to HEAD and layout reset. Fit includes every loaded card, even in long histories, and keeps lane headings separate at overview scale. The overview keeps its loaded range stable; use Load more explicitly there, or zoom in and pan near the frontier for automatic loading. Parents and submodule gitlinks are separate. Narrow windows collapse commit details first and provide an explicit expand control.

## Notes

- Reset requires a clean working tree and index, including no untracked files or in-progress Git operation. Commit or stash changes first. The check runs before and after Fetch; Reset still replaces local commits with the remote branch and requires confirmation.

- Right-click a project in the sidebar to open its directory or launch a terminal there (PowerShell on Windows, Terminal on macOS, `x-terminal-emulator` on Linux). Branch switching, Fetch, Pull and Push are available in the repository header. Reset is in the remote’s **⋯** menu. The workspace menu contains batch actions, recent directories and operation logs. Batch switching previews the parent repositories and their default branches before confirmation.

- Git action buttons modify repositories as indicated. Fetch All updates remote-tracking references without merging or changing working-tree files.
- Moving nodes only changes their visual position for the current application session; it does not reorder or modify Git history.
- Scanning only checks the direct children of the selected parent directory; it does not recursively search nested directories for repositories.

For development setup, test commands, and the P0/P1 E2E coverage matrix, see [DEVELOPMENT.md](DEVELOPMENT.md).
