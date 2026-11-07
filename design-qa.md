# Git Master design QA

Final result: accepted — independent round 16, 2026-11-07, including the revised Canvas06/07.

All 15 source boards and their finite depicted states are accepted. The completed inventory is in `design-state-checklist.md`; the latest mapping is in `test-results/navigator/independent-round16/source-state-evidence-index.md`. Revised Canvas evidence includes direct card-boundary routes, no external dots/ports/timeline, adjacent HEAD reference labels, gray gitlinks, readable blue unloaded-parent continuation, and disjoint cards before and after history loading. The other 13 boards retain their unaffected round-14 evidence.

Final verification includes 72 passing Rust tests, test/production builds, and independent desktop/Git checks. The complete graph preserves all 426 parent edges and 210 gitlinks across 427 real commits; all 428 final cards are disjoint and all 404 common card positions remain unchanged on append. The old full 99-case suite was not run. The test environment is released with owned resource counts at zero.

The records below preserve implementation and review history; earlier blocked results and the old Canvas source acceptance are superseded by the final round-16 record.

## Initial implementation baseline — superseded

Previous acceptance is withdrawn. The implementation must match all 15 source boards and their substates; feature coverage and native-dialog substitution do not satisfy visual acceptance.

The complete required-state inventory is in `design-state-checklist.md`. All source images and interaction rules were opened in the current implementation session. Every row still requires fresh matching-theme, matching-viewport and matching-state evidence, including focused comparisons for dense content.

Current work includes shared production/test confirmation cards, named risk details and batch preview tables; application-owned blocked/recovery panels; independent bounded conflict index-stage views; log-window controls and buffered notices; dedicated empty/loading/error/placeholder views; compact expandable batch errors; and revised overview/canvas composition. These are implementation changes, not a completed acceptance claim.

Initial unit verification: 65 Rust tests passed, including comparison-context ownership, complete long-history fit, primary-action contrast, conflict Pop retention, independent conflict-side assertions, and unchanged unmerged index/worktree after preview. The prior full 99-case desktop suite was not resumed.

Evidence from the first implementation pass is in ignored `test-results/navigator/current/`. It is incomplete, particularly paused logs, all board 12/13/14 substates, hover/focus/disabled states and placeholder selection. Independent review will be recorded here after the application revision is frozen.

## Independent frozen-build review — 2026-11-06

Result: blocked. Fresh guest desktop evidence is under ignored `test-results/navigator/independent/`, with per-state screenshots, source comparisons, view trees, a capture inventory, and synthetic Git evidence. All 15 originals and their interaction rules were opened. This review does not approve all substates or the production executable.

Confirmed blockers:

- P0: conflicting Stash Pop removes the stash reference while unmerged index entries remain; UI/log report success. See `boundaries/14H-pop-after-click.png` and `pop-conflict-git-evidence.txt`.
- P1: remote-more menu collapses into vertical text; paused log row moves 64px when buffered-lines banner appears; ordinary buttons cannot be reached by Tab from repository search; confirmation risk text clips; initialization error pushes recovery controls below the visible details panel.
- P2: unborn/detached labels are incorrect; append-history failure is classified as success; source layout/typography and metadata controls remain mismatched; binary/large previews, logs, and batch results still lack required visual treatment. Narrow expanded details clip commit titles/chips.

Eight empty states, four confirmations, independent conflict-side folding, real partial batch failure/retry, paused logs with real operation-generated new lines, read failure, and date switching have fresh captures. Individual functional passes do not waive visual differences. Outstanding evidence includes initialized out-of-range placeholder details, complete DAG parent-edge checks, all layout/scroll invariants, every tooltip/disabled/menu action, and remaining boundary cases in `design-state-checklist.md`. Previously captured or rejected states are not counted as passes.

No application source changed during independent review. All test applications were closed; capture task was Ready and owned application process count was zero when the guest was released. Fixes and fresh revalidation are required before acceptance.

## Independent review round 2 — 2026-11-06

Result: blocked. New evidence is in ignored `test-results/navigator/independent-round2/`. Both frozen executables were checked; the production executable was separately run in the guest with isolated settings and genuine repository data. It uses the shared application-owned confirmation card.

Verified improvements include conflict Pop retaining the original stash OID, real buffered logs preserving the same screen anchor (y267 to y267), ordinary filter controls working with Tab/Space/Enter, wrapped Reset risk text, scrolling placeholder details with fixed recovery actions, real submodule initialization success, correct unborn/detached labels, truthful failure/skipped outcomes, binary/truncated preview states, and long repository/branch tooltips. A loaded graph comparison found 402 expected and 402 actual parent edges with none missing. A stabilized drag/pan/zoom/append check retained x462/y390/w300/h82 for the measured node. These are limited passes, not whole-board approval.

Remaining P1: pressing Enter on Cancel closes the Stash deletion prompt on KeyDown, but KeyUp reactivates the underlying deletion button and reopens it. `focus-repro/prompt-key-states.json` and held/released screenshots prove the transition; the production executable shows the same final behavior, while Esc cancels. The stash remains intact. Remaining P2 differences include menu icons/danger styling, persistent chrome/footer and change-total colors, and explicit initialization failure/retry presentation.

The next review must still cover the unobserved boundary and action cases: all copy/open/terminal targets, complete immutable diff interaction, pending repository reads and retained refresh content, after-append merge-parent checks, all narrow menus/disabled reasons, all confirmation keyboard sequences and themes, log bounds/read-failure recovery/rotation and row-copy interactions. Invalid timing, hidden semantic nodes, fixture errors, and unexpanded/occluded captures were excluded rather than counted as passes. No claim of complete 15-board acceptance is made.

No application source was modified. Guest released: capture task Ready, combined owned test/production process count zero.

## Independent review round 3 — 2026-11-06

Result: blocked. New frozen-build evidence is under ignored `test-results/navigator/independent-round3/`. This was a targeted review; the old full desktop suite was not run and no application source was changed.

The test and production executable checksums matched the handoff. In both builds, light/dark Stash confirmation passed real Enter/Space held-and-released cancellation, Esc cancellation, and Tab/Shift+Tab return to Cancel. The actual production application used the shared styled card without RPC. Actual confirmation deleted disposable stash data only after key release. Test-build conflict Stage cancellation preserved the unmerged index; actual confirmation staged it. These paths close the observed R2-01 defect, without approving every confirmation type.

New observations also verified full commit hash/path clipboard output, immutable commit-file preview without changing file bytes/index/HEAD, an initial detail-read skeleton, and a retained diff during same-repository refresh (identical painted pixels). Switching repositories and then entering the changes tab showed only the new repository's diff. Real submodule initialization failure displayed the explicit error/retry state; copying the source and retrying after fixing the actual cause succeeded.

Two P2 visual issues remain: the remote menu's Reset target overflows its background and is clipped by the window for an ordinary feature branch, at both wide and narrow sizes; after initialization failure in light theme, the usable Retry initialization button has white text on light gray (measured contrast about 1.28:1). Icons, dangerous-action red, blue branding, footer date, change-total colors, and explicit retry/error semantics improved, but those fixes do not imply whole-board acceptance.

The round-2 unobserved checklist remains open except for the specific additions above. In particular, after-append merge/gitlink edge comparisons, real log bounds/read-failure recovery/rotation and row-copy, all action targets, all branch/reset/batch/blocked confirmation sequences, and precise matching-theme typography/layout still require evidence. No new long-graph or log acceptance is claimed. Captures with wrong test preconditions, summary-only semantic trees, or intermediate loading states were excluded from visual approval.

Guest released: capture task Ready, owned application processes zero, owned input controllers zero. No shared supervisor was stopped and the guest was not restarted.

## Independent review round 4 — 2026-11-06

Result: blocked by a new P1. Both frozen executable checksums matched. A real click on Reset in a clean, ahead-one `main` repository makes both the test and production application exit before the confirmation card appears. Both native logs report that the same element received a tooltip more than once. The production run had no RPC or test prompts. HEAD, index, and file content stayed unchanged in both reproductions. Source inspection identifies the conditional disabled-reason tooltip and unconditional target tooltip on the same Reset element; entering confirmation sets busy and activates that combination.

The long Reset target is visually contained and its complete tooltip is visible in the tested wide/light state. Other widths/themes/long remotes were not reached after the crash. Initialization's released failure state now retains blue fill and readable white text (measured 5.10:1), closing the observed persistent gray-button problem. Its light pressed state still applies opacity to the entire element: measured background (37,132,208), solid text (222,237,248), contrast about 3.32:1. A background-only pressed style would preserve text contrast. Candidate keyboard-focus captures did not prove focus on the primary action and were excluded.

Fresh evidence is under ignored `test-results/navigator/independent-round4/`, including native crash logs, data invariants, same-substate menu comparison, real held/released button screenshots, and measured colors. The narrow search check, long-graph after-append merge/gitlink comparison, log bounds/recovery/rotation, multiple-ref safety and remaining action targets were not completed: the handoff explicitly called for prompt release after a clear new blocker. They remain open, not failed by assumption or counted as passes. Earlier valid limited evidence remains applicable; complete 15-board visual acceptance is not claimed.

No application source was changed. Guest released: task Ready, owned applications zero, owned controllers zero; shared supervision preserved and guest not restarted.

## Independent review round 5 — 2026-11-06

Result: blocked by a new P2 in Fit content for long graphs. Both frozen executable hashes matched. The previous Reset crash is closed in the observed test/production paths: both themes open and cancel normally, and genuine confirmation in production resets the disposable repository to the exact recorded remote OID. The primary initialization button's actual pressed state now measures 6.71:1 in both themes, with default/released at 5.10:1.

New complete graph evidence includes 211 main and 216 submodule commits after append: all 426 expected parent edges (including both real merge parents) and all 210 expected gitlinks exactly match, with no missing or extra edges. All 407 shared graph nodes retain identical painted positions and dimensions across moved/zoomed append. A separate settled real Fit click reaches 15% but leaves the content outside the viewport: 404 graph nodes span y325–3695 while the canvas ends at y872, and only 68 nodes fully fit. The Fit calculation's 15% minimum explains the result; fit-to-content should calculate a complete overview independently of the manual zoom minimum.

New actual Fetch logs contain 2600 real tags and a completed operation END. The original 463272-byte file is displayed as 262016 bytes/1472 lines with the bounded-tail notice. Paused read failure and automatic recovery retain the same row/index and y298 throughout. Actual per-row clipboard copy, native log-file selection, exact full displayed-text copy, same-file truncation to 5163 bytes/29 nonempty lines, and return to today's file are verified. The shorter-than-2000 displayed result does not independently prove the 2000-short-line boundary; rename rotation and the bottom button's actual click remain open.

Fresh evidence is in ignored `test-results/navigator/independent-round5/`. Invalid empty-change fixture setup, stale Fit frames, unavailable observation-node IDs, and unproven keyboard-focus candidates were excluded rather than treated as application failures or passes. The remaining per-board checklist still requires comparison-reference/no-upstream/Fetchtime checks, Apply/Pop and multi-record safety, actual external action targets, remaining empty-state recoveries and confirmation/disabled/narrow/visual states. Complete 15-board visual acceptance is not claimed.

No application source changed. Guest released: task Ready, owned application and input-controller process counts zero; shared supervision preserved and guest not restarted.

## Independent review round 6 — 2026-11-06

Result: blocked. New P1: clicking the visible branch-menu search box immediately closes the popover instead of focusing input. Stable bounds and actual mouse input reproduce this in both test and production executables, in light and dark themes; production used no RPC. The outside-click dismissal boundary around the deferred branch popover needs review. New P2: selecting origin/main for comparison correctly changes counts to ahead 1 / behind 0, but the caption still says main · backup and the backup row remains marked Comparing. The comparison context should follow the selected reference, while keeping operation-remote roles clear.

The previous long-graph Fit issue is closed in eight observed combinations: initial 404 nodes and after-append 428 nodes, wide/narrow, light/dark. Every card fits and the overview remains stable for two seconds. Separate lane headings no longer overlap the miniature graph. Earlier complete edge and append-position evidence remains relevant. Pixel-rounded zoom assertions did not establish all button/wheel continuity, so that narrower item stays open rather than being called a product defect or pass.

New real short-tag Fetch output is 140878 bytes/2608 lines, independently exercising the line cap below the byte cap. The view retains 2000 logical line entries (1999 splitlines plus a terminal blank), with the bounded notice and correct final tag. Actual bottom-button input resumes following; rename/replacement rotation reads the new 919-byte tail and restores the original file afterward. Earlier log-byte/recovery/copy evidence is preserved.

Both comparison references were checked against actual Git counts without changing upstream configuration or HEAD; a genuine Fetch changed the relevant last-fetch time. Alternating two stash OIDs for preview left HEAD, index, status, file bytes and stash list unchanged. Successful Apply retained both records; successful Pop removed only the selected newer record and restored correct tracked/untracked content. Actual directory, terminal cwd, external editor path/content, and child-module navigation targets were independently verified, including a repository path with spaces and an ampersand. All external test windows and processes were closed.

Fresh evidence is under ignored `test-results/navigator/independent-round6/`. Remaining work includes the repaired branch search and safe branch actions, batch copy/open/only-failed retry log comparison, long-conflict external recovery, remaining empty-state actions, theme/selection/scroll persistence and the still-unobserved exact visual/control states. The branch-search blocker prevented later steps in the combined UI script; those were not counted as executed. Complete 15-board visual acceptance is not claimed.

No application source changed. Guest released: task Ready; owned application, input-controller, external-editor/terminal process and Explorer-window counts all zero. Shared supervision preserved; guest not restarted.

## Independent review round 7 — 2026-11-06

Result: blocked by a new P2. Enabled Stage, selected-file Stage, and Save current changes buttons lose their visible labels on hover. In both test and production builds and both themes, the interior becomes a single solid color: light RGB(40,103,168), dark RGB(114,174,230), measured label/background contrast 1.00:1. Default labels are readable; hover changes the background to the same accent color already used by the foreground. The shared worktree primary-button helper needs paired hover foreground/background colors. Git state did not change during the focused hover proof.

R6-01 is closed in the observed paths: both display and checkout branch searches work with actual mouse input in test and production, light/dark, show only the matching feature row, copy the entered query correctly, and close with Esc. R6-02 is closed for the actual origin/backup comparison sample: selected reference, caption, comparison marker and counts now agree; upstream and HEAD remain unchanged.

New safety evidence includes minimum-one branch protection, keyboard filtering without checkout, dirty-worktree blocked-checkout cancellation in both themes, actual confirmed deletion of a merged branch, preservation of an unmerged branch with recovery to history, and genuine safe checkout of a long branch after cleaning only the disposable fixture. Wide/narrow and light/dark long-remote/branch menus and full target tooltips are captured. Narrow repository search supports actual input/copy/clear without overlapping the hint. Zero-search and zero-filter recovery actions were actually executed.

Fresh evidence is under ignored `test-results/navigator/independent-round7/`, including default/hover comparison crops and color measurements. PASS in the capture script is not visual acceptance of the hover defect. After confirming the new blocker in production, the review released the guest as requested. Batch only-failed retry log comparison, long-conflict external recovery, remaining eight empty-state actions, theme/scroll/restart persistence and the unobserved control/visual substates remain open. Previous complete edge/log/system-target/Stash evidence was retained, not rerun or expanded into whole-board approval.

No application source changed. Guest released: capture task Ready, owned application and input-controller process counts zero. Shared supervision preserved; guest not restarted.

## Independent review round 8 — 2026-11-06

Result: blocked by two new P2 findings. Primary inline Stage, selected-file Stage and toolbar Save have no distinguishable keyboard focus: actual Tab traversal in test and production, light and dark, produces pixel-identical default/focused button crops in all 12 combinations, while adjacent ordinary controls visibly gain focus borders. A separate genuine long merge conflict produces content painting across the CURRENT/INCOMING section titles; focused light/dark screenshots confirm the overlap at the normal wide viewport. These are concrete observed states, not a blanket font-size objection.

R7-01 is closed for all three observed entrances in both builds and themes: default, hover and actual held mouse-down preserve opaque white labels. Measured white/background contrast is 5.10 by default and 6.71 on hover/pressed. Git state remains unchanged during the visual proof.

New genuine operation evidence covers a 13-row batch with success/failure/skips, actual clipboard copy and opening the failed repository, both only-failed and per-row retries with operation-log deltas proving only the failed repository ran, and narrow scrolling through the last row plus collapse. Long-conflict previews preserve the complete file/index and support independent folds; actual external resolution, explicit staging confirmation, external merge completion and refresh return to a clean Git state. The visual overlap still blocks acceptance of that preview.

Empty-state actions actually recover unopened/recent workspaces, empty-workspace rescan/change, repository selection, clean refresh, central empty-Stash Save and file selection; prior zero-search/filter recovery evidence remains valid. The unborn terminal/first-commit path remains unexecuted. Theme switching preserves the selected detail and the exact visible history-row geometry; a restart restores the saved theme. Restarted selection/scroll persistence is not claimed.

Fresh evidence is under ignored `test-results/navigator/independent-round8/`, including focus comparisons, long-conflict overlap crops, real retry log differences and before/after empty-state recovery. The first batch and native-folder attempts had test precondition failures corrected in focused continuations; those failures are not presented as application defects or whole-script passes. Full 15-board acceptance is still open, including the two findings and the finite unobserved substates. Existing complete edge/log/system-target/Stash evidence is preserved.

No application source changed. GUEST_RELEASED=true: capture task Ready, owned application and controller counts zero, no new external terminal/editor/Explorer target remains. Shared supervision preserved; guest not restarted.

## Independent review round 9 — 2026-11-07

Result: blocked by a new P2 in the real repository-read error state. The dark-theme Retry button uses solid white 16px text on RGB(114,174,230), yielding measured contrast 2.36:1 in both test and production; the light counterpart measures 5.84:1. The actual retry works after restoring the missing fixture path. Full screenshots and measured crops are under ignored `test-results/navigator/independent-round9/`.

R8-01 fixes are independently verified with real keyboard traversal, both builds and themes: inline Stage Tab15, selected Stage Tab16, toolbar Save Tab13 all show a distinct focus ring without moving the button. Correction to round 8: the previous selected-Stage Tab18 sample was unfocused, so that individual old comparison is withdrawn; it must not be used as focus-defect proof. The other two old entrance observations remain valid. Current default baselines explicitly focus the search field first.

The long-conflict overlap is closed in the observed test light/dark and production light states: bounded panes, independent headers and recovery instructions no longer overlap. Ordinary wheel evidence proves stage2 and stage3 scroll independently. Current-file truncation notice is readable. Some oversized-wheel automation attempts changed outer positioning; these are not reported as product failures. Production's image named dark still shows a light frame and is rejected for dark coverage; side images named end-note do not yet prove both side truncation notes were visible.

New complete recovery evidence covers the unborn center-terminal action, verified terminal cwd, an actual first commit in that terminal, exit and application refresh to real history. Beta detail pending shows its skeleton without the previous Alpha preview. A genuinely missing indexed repository path displays a scoped error; restoring the path and retrying works, and the error-view Change workspace action also opens the native selector and scans a different real workspace. Read gates are released in finally blocks.

All 15 boards remain subject to the finite source-state checklist. Initialization-running cross-repository/disabled states, real batch safety preview/execution, reliable canvas zoom and actual append pending with the new history gate, remaining blocked recovery targets and matching source substates are still unexecuted. Prepared scripts are not counted as tests. Prior valid complete groups are retained without rerunning them or expanding their claims.

GUEST_RELEASED=true: capture task Ready; owned apps, input controllers and NoExit terminal processes all zero. Shared supervision preserved; guest not restarted. No application source changed.

## Independent review round 10 — 2026-11-07

Result: blocked by two concrete P2 modal findings. A real protected checkout to a 73-character branch produces a title extending beyond the 480px blocked card and outside the narrow client window. Test and production, both themes and wide/narrow observations reproduce it. One focused test-light narrow capture showed the wrong window and is rejected; the same frozen-build combination is supported by an earlier valid capture from this round. Separately, an unmerged-branch deletion preflight opens its blocked card while the original branch picker remains on top, obscuring the title and repository fields. This second finding was directly observed in the test build.

R9-01 is closed: real missing-path Retry in both builds and themes has white text on the accepted blue default/hover/held fills, with measured contrast 5.10/6.71. Actual keyboard Retry focus is Tab13 in this error view and shows a clear ring; nearby Tab14 is Change workspace. Restoring the path and clicking Retry recovers in both builds without changing HEAD.

New genuine evidence covers initialization during cross-repository browsing, mutual-write disabled reasons, real initialization failure with long error and fixed narrow footer, exact source copy and successful retry after repairing the source. A four-parent batch safety preview supports cancel-first Enter/Space and real confirmed execution: one safe parent switches; dirty content, in-progress merge index and child branch remain protected. Actual terminal identity repair recovers blocked Save; Pop conflict retains its exact stash OID and View conflicts preserves the conflict index. Checkout/View changes and unmerged-delete/View history targets work with real keyboard recovery.

Canvas measurements now prove 100→120→144 percent button zoom and approximately135 percent after a real wheel event. A real history-read gate captures append pending with existing nodes/details retained; release loads the actual referenced commit, increases regular nodes401→428 and preserves exact node geometry. Switching repositories and returning preserves the observed layout. Rename/long-line horizontal evidence is saved with limitations around end alignment and partition-status interpretation; it is not whole-state approval.

New evidence is under ignored `test-results/navigator/independent-round10/`. Full 15-board acceptance remains open for the two findings and the finite unobserved source substates; existing complete graph/log/system-target/Stash/empty/theme evidence remains valid within its original limits and was not rerun. No application source changed.

GUEST_RELEASED=true: task Ready; owned apps, controllers, upload helpers and NoExit terminals all zero. Shared supervision preserved; guest not restarted.

## Independent review round 11 — 2026-11-07

Result: blocked by P2 R11-01, an actual partition-status mismatch. A genuine staged rename followed by an unstaged content modification produces Git porcelain RM, cached name-status R100 and working name-status M. Both test and production show R in both file groups. Selecting each group shows the correct separate rename/modification diff, so the working row cannot be justified by an aggregate file status. HEAD, index, status, diff facts and file bytes remain unchanged during preview. The loader merges index/worktree flags into one status string, which both partitions reuse.

R10-01 and R10-02 are closed in independently observed final builds: long blocked titles wrap fully inside the card with reachable actions; unmerged-delete preflight leaves no source branch menu above the modal. The footer now correctly shows red failure rather than green preparation text. Actual keyboard History recovery works and the branch remains. Test and production captures and Git safety proof are under ignored `test-results/navigator/independent-round11/`.

The two index-side truncation notices are now directly visible at the ends of their independent previews, alongside the ordered recovery steps. A fresh production capture is genuinely dark and shows no conflict-pane overlap; old mislabeled light frames were not reused. Complete file/index safety is retained.

All 15 source boards are still pending final acceptance of the remaining depicted substates and exact evidence mapping. No arbitrary theme/size Cartesian expansion is required. Existing complete graph/log/system-target/Stash/empty/theme groups remain valid within their original scope and were not rerun. Uninitialized canvas initializer, remaining special-content/reference states, exact date-switch evidence and other explicitly unobserved source details are not marked passed.

GUEST_RELEASED=true: task Ready; owned apps, controllers, upload helpers and NoExit terminals all zero. No application source changed. Shared supervision preserved; guest not restarted.

## Independent review round 12 — 2026-11-07

Result: blocked by P2 R12-01. A real configured origin with no fetched remote refs is correctly listed in the remote table, but the comparison caption incorrectly says “No remote / not compared.” Both final test and production reproduce the contradiction. An actual first Fetch in production creates the real refs and restores the correct origin/main comparison; HEAD, index and remote/upstream configuration remain unchanged. Evidence is under ignored `test-results/navigator/independent-round12/`.

R11-01 is closed with genuine RM and AM fixtures: both builds paint staged R/A and working M/M, with the correct separate preview contents. Git status, HEAD, full index and file bytes remain unchanged. One initial assertion incorrectly treated the read-only snapshot tree as containing only the currently painted partition; the actual screenshots were correct, and that test premise was corrected.

New source07 evidence covers the uninitialized canvas/placeholder, scrollable metadata with full recorded reference, exact source copy, readable primary hover/held, cancelled hold without initialization, and actual initialization into real history through the right-hand action. The extra lane action was observed but not clicked. A non-blocking layout observation remains: its inline button clips the short “submodule” role suffix in the lane title.

A genuine paused rebase now supplies the actual banner, detached HEAD, independent index sides, explicit staging confirmation cancellation, banner-terminal cwd and refresh-without-mutation evidence. Pull/Push tooltips currently cite detached HEAD rather than a rebase-specific reason; the report states that scope explicitly. The terminal exited cleanly.

The finite source-state matrix remains open for the new caption defect and unobserved depicted details, including nested reference ownership, binary/large previews, visible date switch, remaining menu/row/focus details, stash empty partitions/concurrent OID safety and long-line tail reading. No arbitrary theme/size Cartesian expansion or rerunning of valid complete groups is required. Prepared ideas and source inspection are not counted as execution.

GUEST_RELEASED=true: task Ready; owned apps, controllers, upload helpers and NoExit terminals all zero. No application source changed; shared supervision preserved and guest not restarted.

## Independent review round 13 — 2026-11-07

Result: blocked by new P2 R13-01. After closing the display-branch search popup with Escape, Ctrl+K no longer focuses repository search in both final test and production builds. Typing beta leaves the search empty and both repositories visible; a real mouse click on the search field immediately makes the same query work. Test repetition confirms the failure. No Git HEAD mutation occurred. Other popup close routes and the Checkout menu are not claimed to reproduce it. The identical repository-row Tab04–09 frames are rejected as keyboard-focus evidence.

R12-01 and the source07 clipped role are closed. Actual lane initialization now reaches real module history and preserves the parent HEAD/index. New accepted evidence covers real nested remote ownership, binary/large preview notices, full long-line tail reading, three empty Stash partitions, concurrent exact-OID Drop safety, and an actual log-file date switch with paused buffering, read failure and automatic recovery. The date fixture uses the existing test date hook and actual Fetch output, without changing the guest clock. Neutral Fetch keyboard focus is genuinely visible at Tab10; row hover/selection/full tooltip and menu all/current/outside actions also passed.

Evidence is under ignored test-results/navigator/independent-round13. Valid previous full groups remain accepted. The final representative source01–06/15E visual capture was prepared but not run, and the source-state index remains partial until R13-01 and those explicit final mappings are closed. No arbitrary theme/size Cartesian expansion is required.

GUEST_RELEASED=true: task Ready; owned apps, controllers, upload helpers and NoExit terminals all zero. Shared supervision preserved; guest not restarted. No application source changed.

## Independent review round 14 — 2026-11-07 — ACCEPTED

R13-01 is closed independently on both final builds. Display and Checkout popup search, Escape, visible trigger focus, Tab and actual Ctrl+K repository filtering pass in both themes in the test build; production repeats the actual filtered Display/Escape/Ctrl+K path without RPC. Trigger toggle and outside-close keyboard continuation also pass. The earlier production-coordinate setup mistake was corrected before acceptance.

All finite remaining representative views are captured: the same visibly selected commit in dark/light with equal row geometry; dark three-group changes; a real saved Stash in dark; light dirty and clean overview with module; dark selected canvas; dark narrow layout with actual detail expansion and collapse. Dark and light repository-row keyboard focus is visible. Transient obscured light-row captures are excluded; settled native-window diagnostics and actual subsequent filtering provide valid evidence. No failed Enter-selection assertion is counted as passed.

Source boards 01–15 are accepted against the design guide with real Git semantics taking priority over generated sample text/counts. Valid earlier complete evidence is retained. The final source-state evidence index under ignored test-results/navigator/independent-round14 contains 39 finite state rows and 156 verified existing evidence files, with a hash/dimensions manifest. The checklist is reconciled below this final acceptance; earlier blocked records remain historical. No arbitrary theme/viewport Cartesian expansion or rerun of complete functional groups was introduced.

GUEST_RELEASED=true: task Ready; owned apps, controllers, upload helpers and NoExit terminals all zero. Shared supervisor preserved; guest not restarted. No application source changed by the verifier.

## Independent review round 15 — 2026-11-07 — BLOCKED

Only the revised Canvas boards 06/07 are reopened. The other 13 boards and unaffected business evidence retain round-14 acceptance. New source hashes and fresh source/production comparisons are recorded under ignored test-results/navigator/independent-round15; old Canvas comparisons do not satisfy this revision.

New P2 R15-01: an out-of-range module placeholder is placed at exactly the same initial rectangle as a different already-loaded module tip. Test light/dark and production light reproduce it without a user drag. Both rectangles are world (416,124,236,64), screen (772,387,236,64). The placeholder hides the real tip and appears to emit its green parent line. A real 206-commit module and an older parent gitlink reproduce this; HEAD/index/status remain unchanged. Full append in the long fixture also leaves two loaded commits coincident. Boundary routing and complete edge sets cannot substitute for visible, separate nodes.

Accepted increments: no external dots/ports/side timeline, card-boundary parent and gray gitlink routes, a non-emitting HEAD reference, true merge parents, actual separated drag/pan/zoom/Fit/reset, and unchanged conventional history-list dots. The appended real graph has 427 commits, 426 parent edges and 210 gitlinks, with no missing/extra relation, finite routes and no route through card/label interiors. All 404 common card positions and pan/zoom persist across actual append. Real test and production ordinary dark/true-uninitialized light screenshots are retained.

The blue-unloaded-continuation probe did not frame the actual target card/blue dashed segment readably, so its script PASS is not visual acceptance. It remains a finite follow-up after R15-01 is fixed. No unrelated board or full suite was rerun.

GUEST_RELEASED=true: task Ready; owned apps, controllers, upload helpers and NoExit terminals all zero. Shared supervision preserved; guest not restarted. No application source changed by the verifier.

## Independent review round 16 — 2026-11-07 — ACCEPTED

R15-01 is independently closed on both final builds. The actual loaded module tip is visible and a real click opens its correct OID/details. The exact original fixture is copied privately, not replaced by invented graph state. Initially 206 cards and after actual append 210 cards are pairwise disjoint, with all 206 common rectangles preserved. HEAD/full index/status remain unchanged.

The complete real graph is pairwise disjoint initially (404 cards) and after append (428 cards). All 427 real commits, 426 parent edges and 210 gitlinks are retained, exactly matching Git, including both real merge parents. All 404 common card rectangles and actual pan/120% zoom remain unchanged. Routes attach at the required card sides with finite orthogonal points and no card/HEAD body intersections. Actual Fit and narrow Fit complete.

The finite remaining source07 blue-parent continuation is now visibly readable at 100% with the actual unloaded main-lane card fully inside the viewport. The evidence records the full OID and final bounds; only native drags and the existing genuine history-read gate were used. The gate is released in finally. Earlier unframed or out-of-canvas probe captures remain excluded. Final test and production dark normal/light uninitialized representative frames are fresh against the revised source hashes.

Latest revised boards 06/07 are accepted. Other 13 boards retain R14 and unaffected R15 routing/interaction/list observations retain their exact scope. The latest all-15 evidence index and checklist are reconciled; R15 blocked records remain historical. No full old suite or arbitrary matrix expansion was introduced.

GUEST_RELEASED=true: task Ready; owned apps/controllers/upload helpers/NoExit terminals all zero. Shared supervisor preserved; guest not restarted. No application source changed by the verifier.
