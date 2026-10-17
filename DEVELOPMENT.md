# Development

## Rust checks

Use a current Rust stable toolchain (edition 2024), Git, and the GPUI platform dependencies described in README.md.

```sh
cargo fmt --check
cargo check
cargo check --features test-rpc
cargo test
cargo test --features test-rpc
```

## End-to-end tests

The former Shell suites have been migrated to Python 3.12+ and pytest. The tests launch a real GPUI executable, click controls with desktop mouse input, observe the rendered view tree through loopback JSON-RPC, and assert the actual Git index, files, branches, stash entries, and local remotes.

Install dependencies:

```sh
python -m pip install -r tests/requirements.txt
```

On an unlocked Windows desktop:

```sh
python -m pytest tests/e2e -v --junitxml=test-results/e2e.xml
```

The suite builds a dedicated `git_master_e2e.exe` once, leaving an already running development executable alone. It moves the mouse and brings its own application windows to the foreground. Avoid interacting with the desktop during the run. Run tests serially; do not use pytest-xdist against a shared desktop.

On Linux/X11 install the GPUI build dependencies plus `xvfb`, `xauth`, `xdotool`, `scrot`, `gnome-screenshot`, and `python3-tk`, then run:

```sh
xvfb-run -a -s "-screen 0 1600x1200x24" python -m pytest tests/e2e -v --junitxml=test-results/e2e.xml
```

To use an existing test-rpc build:

```sh
cargo build --features test-rpc
xvfb-run -a -s "-screen 0 1600x1200x24" python -m pytest tests/e2e --app-binary target/debug/git_master
```

Use `--app-binary target/debug/git_master_e2e.exe` on Windows to reuse a prior dedicated build. Select only critical cases with `-m p0`, or core workflow cases with `-m p1`. Specify `tests/e2e` explicitly before custom options so pytest loads the suite's options. macOS desktop input and Wayland are not currently implemented by this driver.

Tests use per-test temporary repositories, synthetic identities, disabled personal/system Git configuration, local-only remotes, and an individual loopback port. Each application process is terminated during fixture teardown. Timeouts poll observable conditions; they do not assume operations finish after a fixed sleep. Test code must not operate on a developer's repositories.

## P0/P1 coverage

P0 means data integrity or explicit approval before a destructive action. P1 means a core navigation, history, preview, submodule, or synchronization workflow. These are project test priorities, not a claim that every UI interaction has exhaustive coverage.

| Priority | Workflow | Assertions |
|---|---|---|
| P0 | Stage / Unstage: modified, new, deleted, literal paths, unborn branches | Correct index entries; newest working files preserved; no glob expansion; deleted files stay absent |
| P0 | Save stash, Apply, Pop | Staged/working/untracked data saved and restored; Apply retains stash; successful Pop removes it |
| P0 | Stash deletion | No mutation before confirmation; Cancel preserves stash; Delete preserves working files |
| P0 | Conflicting Apply / Pop | Visible failure; stash and local edits retained |
| P0 | Branch checkout/deletion safeguards | Overlapping local changes and unmerged commits protected; confirmation cancellation respected |
| P0 | Reset to remote | Cancel preserves HEAD; clean confirmed reset matches remote; dirty tree blocks reset and retains edits |
| P1 | Scan / select / switch repositories | Correct ordering, branches and dirty indicators; details/history match selection; no previous repo changes leak |
| P1 | History list / canvas / commit details | Expected commits, graph lanes and links; open and close details via real controls, including long paths |
| P1 | Submodules | Initialized/uninitialized lanes and placeholders; selected module owns history; staging affects only module; initialization works |
| P1 | Git Change / stash previews | Staged/worktree/untracked sections; diff rows and highlight ranges visible; preview does not mutate files |
| P1 | Fetch / Pull / Push | Fetched references without checkout; pull restores autostashed local edits; push reaches local remote; sync counts refresh |
| P1 | Fetch All / Switch All to Main | Failed remote does not prevent others updating; default branches switch; untracked work survives |

### Coverage audit additions

The suite now has 42 cases: 24 P0 and 18 P1. The audit compared the worktree actions, history controls, repository navigation, batch operations, and remote-operation handlers with executable UI tests. In addition to the initial matrix above:

| Priority | Previously missing workflow | Assertions |
|---|---|---|
| P0 | Push rejection / Pull rebase conflict | Remote and local commits remain recoverable; untracked and unsaved work survives; visible failure |
| P0 | Multiple and stale stash entries | Apply/Pop/Delete use the selected object ID after indexes shift; missing entries cannot affect replacements |
| P0 | Stash without identity / locked index | Failure preserves staged and working data; another operation's lock remains untouched; retry works |
| P0 | Submodule stash isolation | Save/preview/Pop target the selected module; parent work and parent stash remain untouched |
| P0 | Batch checkout partial failure / missing reset branch | Unsafe target is preserved; other repositories proceed; missing remote branch does not discard commits |
| P1 | Refresh, empty workspace and clean repository | Disabled batch actions are harmless; external changes update list, sidebar and Info; untracked-only stash preview hides empty sections |
| P1 | Rename, binary and Unicode files | Preview is readable; Stage/Unstage preserves exact file bytes and correct index paths |
| P1 | Branch visibility filter | List and graph reflect selection; last visible branch cannot be hidden; HEAD does not change |
| P1 | Canvas click, node drag, pan and zoom | Click opens details; drag does not; geometry changes; per-repository view state survives switching |
| P1 | Repository context menu / failed Fetch retry | Menu opens/dismisses without selecting the wrong repository; failure is visible and retry recovers |
| P1 | Default branch resolution | Use Main supports local master and a remote default branch without a local counterpart |
| P1 | Native workspace picker (Windows) | Cancel preserves selection; choosing another workspace clears old details and rescans |

The native folder-picker case runs on Windows and is explicitly skipped in Linux CI, which has no desktop portal session. The remaining 41 cases run on both supported test platforms. This is functional coverage, not exhaustive combinations: OS-native dialog appearance, launching external terminals/file managers, clipboard integration, and pixel-level appearance remain separate desktop integration/visual checks. macOS/Wayland input remains outside this driver's supported platforms. Difftastic parsing/fallback variants retain Rust coverage; the E2E diff cases exercise the renderer available on the runner.

`GIT_MASTER_TEST_CONFIG_DIR` redirects libgit2's system/global/XDG/ProgramData configuration search paths in the test build before threads start. Git subprocesses separately use `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_NOSYSTEM`; both configuration readers must be isolated. Production builds do not honor the test-only setting.

## Test observation and confirmation boundary

The test build tracks control bounds during real painting and attaches them to semantic view-tree nodes. Python resolves the test process's client window, accounts for display scale, clicks the rendered control, and waits for the corresponding UI and Git effects. The application state mutation RPC helpers remain for compatibility but the pytest cases do not call them.

For portable confirmation testing, `GIT_MASTER_TEST_PROMPTS=1` installs a custom GPUI prompt renderer **only in a test-rpc build**. Application code still requests and awaits the normal prompt; tests click Confirm/Cancel buttons and exercise the real approval callbacks. The normal build keeps native system dialogs. These tests cover confirmation behavior, not native Windows/macOS dialog appearance, accessibility, or OS input permissions. Screenshots on failure are diagnostics, not pixel-baseline visual regression tests.

## Failure artifacts and CI

`--artifacts test-results/e2e` selects the diagnostics folder. Each case records application output and isolated operation logs. Failures also save the last view tree and a screenshot cropped to the test application's client window where available. A JUnit XML report is emitted by `--junitxml`.

CI builds the application on Linux, Windows, and macOS. The desktop E2E job runs pytest serially under Linux Xvfb and uploads JUnit, application logs, failure trees, and screenshots even after a test failure. Local Windows runs exercise the same tests on a real desktop. No network credentials or external Git hosting are needed by the tests.
