# Design completion checklist

Status: accepted — independent round 16, 2026-11-07. Revised Canvas06/07 satisfy the latest source rules; R15-01 and the readable blue continuation are closed. The other 13 boards retain round-14 acceptance. Latest all-15 finite design acceptance is granted.

| Board | Required states and details |
| --- | --- |
| 01 | Dark history: workspace/search/status filters; grouped parent/submodule rows; branch chips; DAG rows and selection; author/hash/time columns; commit metadata, files, body; task result |
| 02 | Light history: all 01 states; identical geometry and preserved selection/scroll on theme change |
| 03 | Staged/unstaged/untracked groups and counts; same path in both partitions; selected file, status, stage/unstage; real old/new gutters and colored hunks; read-only notice |
| 04 | Stash list/date/branch/title; selected record metadata; save/apply/pop/delete; file summary; staged/working/untracked partitions and empty partition; no mutation on preview |
| 05 | Repository path/copy/open, branch/HEAD/state; change statistics/navigation; remote table/fetch/comparison; upstream explanation; submodule view/init; operation reminder |
| 06 | Dark canvas lane headings, card-boundary parent bottom/top edges, gray dashed right/left gitlinks, adjacent HEAD reference label without its own edge; no external dots/ports/side timeline; selected node/details; zoom/pan/drag/fit/HEAD/reset; append history |
| 07 | Light canvas; uninitialized placeholder selection/details/path/source/reference/init/copy; out-of-range placeholder/details/load; preserve positions on append |
| 08 | Paused rebase banner and refresh/terminal; disabled operation reasons; conflicts group; bounded read-only current file with colored regions; independent collapsible index-stage sides; explicit stage confirmation; ordered external recovery steps |
| 09 | Partial failure warning; actual success/failure/skipped/total summary; table identities; expandable per-item error/open/retry; failed-only retry; copy/collapse; truthful terminal status |
| 10 | Independent log window/title/theme; selected file/refresh/copy/follow; bounded lines notice; paused new-lines banner; actual logs; preserved reading position; bottom follow/directory; read failure/rotation |
| 11 | History multiselect/search/all/current/minimum-one; checkout/search/current/delete; remote picker/refresh/copy/reset; workspace open/recent/fetch-all/switch/logs; repository open/terminal/copy |
| 12 | Unopened workspace/recent; empty workspace/change/rescan; unselected repository; zero search results/clear/reset; clean worktree/refresh; no stashes/save; unborn history/terminal; no file selected |
| 13 | Initial scan/read skeleton; refresh preserves data; scoped read error/path/retry/change; append error/retry/preserved layout/completed; binary/large preview/external; submodule initializing/failure/retry/copy; no remote disabled reasons, detached HEAD, unfetched comparison |
| 14 | Reset named repository/target/real local-ref impact/clean precondition; exact stash deletion; safe merged branch deletion; batch target/safety table; blocked checkout/view changes, blocked deletion/view history, stash failure/terminal, Pop conflict/retained record/view conflicts; cancel initial focus |
| 15 | Button default/hover/pressed/focus/disabled reason; repository hover/selected/focus/long-name tooltip; idle/running/success/partial/failure/skipped; keyboard menus/tooltip/containment; narrow window/reachable details and controls; logs follow/paused/new/read-failure/date switch |

All 15 boards are independently accepted at the latest finite scope. Revised Canvas06/07 use round-16 captures and proofs; the other 13 boards retain round-14 observations. The current index is `test-results/navigator/independent-round16/source-state-evidence-index.md`, with 158 verified referenced evidence files recorded in its `accepted-evidence-manifest.json`. The round-14 index remains historical. Feature tests or source inspection alone were not used as visual acceptance.


## Evidence reconciliation — round 14 retained, Canvas updated in round 16

| Boards | Final evidence / retained observations | Result |
| --- | --- | --- |
| 01–02 | Final same visible selected commit in both themes; equal geometry. Prior immutable file/copy, scroll preservation and theme restart retained. | Accepted |
| 03 | Final dark staged/working/untracked groups; prior RM/AM semantics, actual gutters/hunks and complete long-line tail. | Accepted |
| 04 | Final dark real saved record; prior empty partitions, invariant preview, Apply/Pop and concurrent exact-OID Drop. | Accepted |
| 05 | Final light dirty/clean overview and module; prior actual system targets, nested ownership and unfetched/Fetch. | Accepted |
| 06–07 | Revised source hashes matched to fresh final frames; all initial/appended cards disjoint, true complete Git edges, stable coordinates/pan/zoom, readable blue unloaded-parent continuation. | Accepted — round 16 |
| 08 | Actual paused Rebase/banner/terminal, read-only bounded conflict sides, explicit stage confirmation and external recovery. | Accepted |
| 09 | Actual partial result identities/counts, copy/open, failed-only/per-row retry, narrow scrolling/collapse. | Accepted |
| 10 | Actual bounded log window, copy/selection, follow/pause/buffer/reading position, failure/rotation/date recovery. | Accepted |
| 11 | Actual menus/search/all/current/minimum-one/long-name actions; final Escape/Tab/Ctrl+K, outside and toggle focus continuation. | Accepted |
| 12 | All eight empty-state recovery actions observed in rounds 7–9. | Accepted |
| 13 | Actual pending/preserved refresh/scoped error/retry/append failure/special previews/init/detached/unfetched. | Accepted |
| 14 | Named risk/confirmation cards, cancel focus, real safe/blocked operations and correct recovery targets retained. | Accepted |
| 15 | Real button states, dark/light row focus, six truthful task states, bounded keyboard menus, narrow reachable details and log states. | Accepted |

R13-01 is closed on final test and production builds. Previously rejected or mislabeled focus/theme frames remain excluded. GUEST_RELEASED=true; task Ready and all owned resource counts zero. This records the finite design acceptance, not a guarantee over arbitrary future data/platform combinations.

## Historical Canvas revision — independent round 15 — BLOCKED

New source hashes and final test/production identities are in the round-15 evidence manifest. Revised 06/07 remain blocked: placeholder and distinct loaded tip have identical initial rectangles, and two real commits remain coincident after full append. The other 13 board conclusions stay accepted. The new card-boundary routing, no external nodes, true 426 parent/210 gitlink relation sets and preserved 404 common-node geometry are valid increments, not full visual acceptance. Correct the collision, then obtain the finite readable blue-unloaded-parent frame and reconcile this section. GUEST_RELEASED=true; all owned counts zero.

## Latest Canvas revision — independent round 16 — ACCEPTED

R15-01 is closed in test and production: actual tip visible/selectable; 206→210 card pairs disjoint and common coordinates preserved. The full graph 404→428 is also pairwise disjoint, with all 427 commits/426 parent/210 gitlink relations exact and all 404 common rectangles/pan/zoom stable. The remaining true blue parent-history continuation and unloaded card are readable at 100% and fully inside the actual viewport. New final dark/light production frames use the revised source hashes. The prior round-15 section records historical failure; it no longer describes the current result. Latest source-state-evidence-index.md under ignored independent-round16 supersedes only Canvas acceptance and total status, retaining the other 13 accepted boards and valid prior evidence. GUEST_RELEASED=true; all owned counts zero.
