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

On Linux/X11 install the GPUI build dependencies plus `xvfb`, `xauth`, `xdotool`, `scrot`, `gnome-screenshot`, `python3-tk`, and `fonts-noto-cjk`, then run:

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

The suite now has 85 cases: 26 P0 and 59 P1. The audit compared the worktree actions, history controls, repository navigation, batch operations, and remote-operation handlers with executable UI tests. In addition to the initial matrix above:

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

The two native folder-picker cases run on Windows and are explicitly skipped in Linux CI, which has no desktop portal session. The remaining 83 cases run on both supported test platforms. This is functional coverage, not exhaustive combinations: OS-native dialog appearance, launching external terminals/file managers, and pixel-level appearance remain separate desktop integration/visual checks. macOS/Wayland input remains outside this driver's supported platforms. Difftastic parsing/fallback variants retain Rust coverage; the E2E diff cases exercise the renderer available on the runner.

`GIT_MASTER_TEST_CONFIG_DIR` redirects libgit2's system/global/XDG/ProgramData configuration search paths in the test build before threads start. Git subprocesses separately use `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_NOSYSTEM`; both configuration readers must be isolated. Production builds do not honor the test-only setting.

## Test observation and confirmation boundary

### Failure, cancellation and retry recovery

Four desktop cases exercise rejected branch deletion, rejected checkout, cancelled stash deletion, and repeated failed stash Apply followed by a successful retry. They check history/filter recovery as well as Git refs, index/files and a new completion for each attempt. Three cases fail against the pre-fix executable; the repeated-Apply case is additional recovery coverage. Failed branch mutations retain the previous visible branches and rebuild their history accordingly. Cancellation explicitly publishes a cancelled result and releases busy state.

The six driver acknowledgement checks include both positive and cancelled confirmations disappearing without a new result. Test snapshots are emitted after painting and published by monotonically increasing view revisions; a late or duplicate worker result cannot replace a newer snapshot. A Rust regression forces that out-of-order publication without timing assumptions.

A separate Windows regression holds a completed scan on its worker, opens and cancels the native workspace picker, then releases the result. It fails before the fix that separates directory-prompt ownership from scan-task ownership. `GIT_MASTER_TEST_SCAN_GATE` is confined to the test build and worker thread with a 30-second bound and fixture release in `finally`. The folder-switch case waits for the native dialog to be active, fills and verifies its focused Edit control via standard native control messages, observes the expected folder contents through Windows UI Automation, and clicks Select Folder. Control rebuilds during shell navigation retry only ElementNotAvailableException observations; the expected folder item must still appear. Windows PowerShell/UIAutomationClient is required for these two native-dialog cases; other cases retain normal desktop clicks and read-only application RPC observations.

### Asynchronous navigation regressions

Five desktop cases hold actual Git operations at bounded pre-push, reference-transaction and post-checkout hooks. They load another repository's Change/Stash while Push is pending, preserve another repository's branch filter after deletion, reject a prior history snapshot after A→B→A navigation, and keep same-relative-path submodules isolated by parent identity. Four of these cases fail against the pre-fix executable; the submodule case is an additional boundary check. Release files control ordering, and fixture teardown releases every hook even after assertions fail; no fixed delay determines correctness.

Only these concurrency cases use `await_operation=False`: they require the hook's entered marker and an observable busy state, navigate while the hook is blocked, release it, and finally require a new idle completion. Normal operation clicks keep automatic completion acknowledgement. Hook fixtures are checked out with LF endings on Windows as well as Linux.

Selection and workspace transitions invalidate read-request ownership. A completed old read cannot clear a newer loading state or publish its snapshot. Mutations remain serialized and continue against their original Git target; a superseded navigation causes current details/filter data to be reloaded rather than restoring the operation's previous view. Visible Change/Stash data is refreshed after mutations without overwriting the operation outcome message.

Operation clicks must observe a newer `status_revision` and an idle state before assertions. The revision is emitted by the test build's actual status setter, so identical successive messages still identify distinct operations. Confirmation and cancellation both have the same requirement. Error checks read only `status-message`, never repository paths or the complete tree. Keep independent Git/index/file assertions after this acknowledgement; a completion signal alone does not prove correctness.

Run `python -m pytest tests/unit -q` for driver mutation checks. These deliberately swallow mouse input or remove a confirmation without completing the action, and require the driver to reject stale success/error messages and in-progress states. They are separate from the 85 desktop E2E cases and also run in CI.

The test build tracks control bounds during real painting and attaches them to semantic view-tree nodes. Python resolves the test process's client window, accounts for display scale, clicks the rendered control, and waits for the corresponding UI and Git effects. The application state mutation RPC helpers remain for compatibility but the pytest cases do not call them.

For portable confirmation testing, `GIT_MASTER_TEST_PROMPTS=1` installs a custom GPUI prompt renderer **only in a test-rpc build**. Application code still requests and awaits the normal prompt; tests click Confirm/Cancel buttons and exercise the real approval callbacks. The normal build keeps native system dialogs. These tests cover confirmation behavior, not native Windows/macOS dialog appearance, accessibility, or OS input permissions. Screenshots on failure are diagnostics, not pixel-baseline visual regression tests.

## Failure artifacts and CI

`--artifacts test-results/e2e` selects the diagnostics folder. Each case records application output and isolated operation logs. Failures also save the last view tree and a screenshot cropped to the test application's client window where available. A JUnit XML report is emitted by `--junitxml`.

CI builds the application on Linux, Windows, and macOS. The E2E job also executes the Rust regression suite and driver acknowledgement checks, then runs desktop pytest cases serially under Linux Xvfb and uploads JUnit, application logs, failure trees, and screenshots even after a test failure. Local Windows runs exercise the same tests on a real desktop. No network credentials or external Git hosting are needed by the tests.

The task feedback regression holds a real Push at a Git hook, then rejects another Push with 120 diagnostic lines. It verifies compact and expanded panel bounds, remaining content height, automatic collapse on completion, full clipboard output, and remote Git state. The final diagnostic line has real painted bounds: it must begin outside the details viewport and move fully inside it after a wheel scroll. Screenshots alone are not accepted as scroll assertions. Passing screenshots are saved in that case's artifact directory.

Twelve task-progress cases hold Stage, Unstage, Save, Apply, Pop and Drop on their actual worker boundary, each after a previous success and a recoverable failure. They assert the text of the painted summary, a fresh status revision, collapsed old details, and the final Git/index/file state. `GIT_MASTER_TEST_WORKTREE_GATE` names a test-only gate directory; creating it arms the gate, `entered` acknowledges worker arrival, and `release` resumes it (bounded to 30 seconds). Concurrent reads retain the running mutation's status. The shared worker gate also preserves the existing scan cancellation boundary.

On Windows, if foreground activation is denied, the driver temporarily raises only its own window and clicks a safe header point after verifying that point belongs to the test process; it then restores normal stacking. It never clicks another application's window to obtain focus.

Mutation verification: removing the worktree start-status update makes the Stage-after-failure case reject the stale error summary; disabling vertical scrolling makes the long-error case time out waiting for the painted final line to enter the viewport. Both mutations were verified to fail before restoring the fixes. Async navigation tests wait for the painted busy/status frame before resolving repository-row coordinates, and assert that concurrent Change/Stash reads preserve the active Push summary.

The independent application-log window has its own GPUI entity, request ownership and cancellable read task. `LogViewerHost` reads the current daily file off the UI thread and bounds snapshots to the latest 256 KiB and 2,000 lines. A missing file is an empty state; read errors can be retried. Closing releases the view and cancels publication, and reopening starts a fresh request. Existing log redaction stays at the writer.

Four P1 desktop cases verify separate-window activation without changing the main selection, refresh and clipboard contents, bounded large-log scrolling using painted end-line bounds, missing/read-error recovery, and closing/reopening while a read is held. Windows also exercises native Alt+F4 closure. `GIT_MASTER_TEST_LOGS_GATE` controls the reader via the existing test-only worker gate. `get_logs_view_tree` exposes read-only snapshots after the log window paints; the driver selects windows by process ID plus exact title and uses each window's own bounds. Log fixtures assert paths stay inside isolated test state before changing files, and use unique refresh markers to reject stale snapshots.

The log window polls once per second with at most one read in flight; manual refresh uses the same serialized read path. Polling does not replace content with a loading screen. The framework-independent Viewer model buffers successful snapshots while following is paused, including when the bounded tail rolls over. Resume adopts the latest buffer and requests bottom alignment during layout. Upward wheel input pauses immediately; downward input checks the real scroll handle after GPUI dispatch and resumes only at the bottom. Closing the window cancels both the timer and read publication. All read errors retain the last readable snapshot and buffered update; a fixed-height status line shows retrying errors without moving a paused viewport; polling recovers without a click.

Three additional P1 desktop cases append real log entries without clicking Refresh, assert the painted final line is visible when following, verify upward scrolling freezes both content and position across completed reads and tail rollover, resume by button and wheel, and exercise an automatic read failure/recovery. A worker gate makes the failure setup deterministic. Two Viewer regressions cover buffering/rollover and automatic-error recovery independently of GPUI.

The log review adds four desktop regressions. Automatic and queued manual read errors must preserve both the displayed content and exact painted visible-row position, then recover into the paused buffer. Both cases fail on the pre-fix build (the automatic error inserted a new row; manual error discarded the snapshot). The result gate `GIT_MASTER_TEST_LOGS_RESULT_GATE` holds already-read snapshots by numbered tickets (`armed`, `N.hold`, `N.entered`, `N.release`, `N.completed`), with a 30-second bound and fixture cleanup. An old window's result is released only after a new window has loaded and paused; generation, content and follow state must survive that completion and two later polls. Another case queues three manual clicks while a read is held, observes two timer ticks without overlapping work, then proves the next request originated from the manual queue and buffers its result if the user pauses while it is in flight. Read origin prevents a later automatic poll from falsely satisfying the queued-refresh test; removing queued manual dispatch was verified to fail that assertion. Observation counters and result gates exist only in the test build; input stays native and RPC stays read-only.

Nine rotation/truncation desktop cases cover rename-and-recreate and copy-truncate replacements, plus the December 31 to January 1 daily path transition, both while following and paused. Each starts with a bounded large tail and replaces it with short content, asserting exact content/path, truncation reset, paused painted position and buffered resume. Numbered result gates release the first post-change read while holding later reads, so a later poll cannot repair a bad assertion. A missing new-day file must become empty automatically; a real Stage operation then writes to the new daily file and is followed without changing the previous file. GIT_MASTER_TEST_LOG_DATE is a test-build-only date-file dependency shared by reader and writer; production always uses the local calendar date and no system clock is changed. Date-file updates use atomic replacement. Caching the first daily read path was verified to fail the new-year regression on its first post-change result. Oversized Unicode and ASCII entries must retain the exact valid suffix and put the final wrapped row in view, while all toolbar controls remain inside the window. Large retained tails must expose fewer than 100 actually laid-out rows to prove frame work stays bounded. That case fails on the old executable, which displays an empty log. Rust regressions cover exact byte-boundary lines and oversized entries with no newline, LF and CRLF.

Under Linux/Xvfb the driver raises the PID/title-selected native window before focusing it: with no window manager, focusing the main window alone leaves Logs above it and routes clicks to the wrong surface. The log-window helper also waits for its initial paint and explicitly activates each new native window, matching main-window startup: GPUI can leave unfocused Xvfb windows idle. The new-day writer test exercises the switch in both directions. The CI desktop image installs Noto CJK fonts for the large Unicode log cases rather than weakening their data, content or painted-position assertions.

Large log text is prepared on the reader worker as immutable display rows, each at most 512 Unicode scalars. The GPUI view renders those bounded rows rather than passing the full multiline body or an oversized final entry to a single text element. Empty rows retain line height; Copy displayed keeps the original text without inserted display breaks. The painted end marker identifies the final rendered chunk, including wrapped entries. Host regressions verify empty rows, Unicode boundaries, row size and unchanged copy text.

The log body uses GPUI ListState to measure and paint only visible rows plus a small overdraw region; bounding the total bytes or each text run alone does not bound a frame containing thousands of Unicode lines. Display rows are shared immutable worker output. Replacing a followed snapshot resets list measurements and anchors at the end; a paused snapshot and its list remain unchanged until resume. For offscreen end entries, tests compare the first fully visible real row (index, text and painted y) rather than inventing end bounds. They still require the real final row to be painted inside the viewport after follow or downward scrolling.

X11 wheel input is a batch of native notch events. Pause tests establish their baseline only after the same fully visible row (index, text and painted y) survives two timer-driven painted frames. This settling occurs before changing log files or releasing a held result; post-update assertions remain immediate and strict. Waiting only for following=false can capture the first notch and falsely blame the remaining legitimate wheel movement on an async read.
