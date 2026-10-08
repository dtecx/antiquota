# UI and desktop feature inventory

This inventory records legacy UI and shell behavior at commit `17ec296`. This subsidiary inventory contains 612 acceptance rows, F-398 through F-1009, also copied in full into [FEATURES.md](FEATURES.md). Each acceptance row needs rewrite proof. Existing test references identify legacy tests only; no rewrite pass is claimed. Source locations refer to legacy files.

The full read manifest is [source-read-ui.md](source-read-ui.md). Known broken behavior must be repaired rather than preserved.

## Application foundation

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-398 | [ ] Authenticated JSON calls send X-Antiquota-Token and display API errors as a toast. | `antiquota/ui/app.js:98` | None identified | [Pending](TEST_REPORT.md#f-398) |
| F-399 | [ ] The session event stream uses a token-authenticated EventSource. | `antiquota/ui/app.js:1029` | None identified | [Pending](TEST_REPORT.md#f-399) |
| F-400 | [ ] A reconnected event stream reloads authoritative state after a disconnect. | `antiquota/ui/app.js:1029` | None identified | [Pending](TEST_REPORT.md#f-400) |
| F-401 | [ ] Initial load selects the URL-fragment session when it exists. | `antiquota/ui/app.js:2511` | None identified | [Pending](TEST_REPORT.md#f-401) |
| F-402 | [ ] Initial load otherwise opens the first session or the start screen. | `antiquota/ui/app.js:2511` | None identified | [Pending](TEST_REPORT.md#f-402) |
| F-403 | [ ] A missing initial model list is retried after four seconds. | `antiquota/ui/app.js:2511` | None identified | [Pending](TEST_REPORT.md#f-403) |
| F-404 | [ ] A toast replaces the prior timer and clears after 2.6 seconds. | `antiquota/ui/app.js:111` | None identified | [Pending](TEST_REPORT.md#f-404) |
| F-405 | [ ] Confirmation dialogs support Continue, Cancel, Escape and dangerous-action styling. | `antiquota/ui/app.js:24` | None identified | [Pending](TEST_REPORT.md#f-405) |
| F-406 | [ ] Desktop export uses the native save dialog. | `antiquota/ui/app.js:39` | None identified | [Pending](TEST_REPORT.md#f-406) |
| F-407 | [ ] Browser export creates a downloadable file. | `antiquota/ui/app.js:39` | None identified | [Pending](TEST_REPORT.md#f-407) |
| F-408 | [ ] External desktop links open in the system browser. | `antiquota/ui/app.js:18` | None identified | [Pending](TEST_REPORT.md#f-408) |
| F-409 | [ ] Clipboard reads use native APIs in desktop mode and browser APIs otherwise. | `antiquota/ui/app.js:19` | None identified | [Pending](TEST_REPORT.md#f-409) |
| F-410 | [ ] Clipboard writes use native APIs in desktop mode and browser APIs otherwise. | `antiquota/ui/app.js:20` | None identified | [Pending](TEST_REPORT.md#f-410) |
| F-411 | [x] Malformed or unavailable localStorage does not prevent preference initialization. | `antiquota/ui/app.js:69` | None identified | [Passed](TEST_REPORT.md#f-411) |
| F-412 | [x] Preference writes tolerate unavailable localStorage. | `antiquota/ui/app.js:96` | None identified | [Passed](TEST_REPORT.md#f-412) |

## Sessions and projects

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-413 | [x] Session rows display title, relative update time and status. | `antiquota/ui/app.js:224` | None identified | [Passed](TEST_REPORT.md#f-413) |
| F-414 | [x] Session rows identify pinned sessions and worktree sessions. | `antiquota/ui/app.js:224` | None identified | [Passed](TEST_REPORT.md#f-414) |
| F-415 | [x] Sleeping sessions show the wake time. | `antiquota/ui/app.js:224` | None identified | [Passed](TEST_REPORT.md#f-415) |
| F-416 | [ ] Sessions awaiting approval display an attention message. | `antiquota/ui/app.js:224` | None identified | [Pending](TEST_REPORT.md#f-416) |
| F-417 | [x] Goal sessions show the current round and limit or final goal state. | `antiquota/ui/app.js:224` | None identified | [Passed](TEST_REPORT.md#f-417) |
| F-418 | [x] Sessions sort by most recent update within project groups. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-418) |
| F-419 | [x] Pinned sessions appear in a separate Pinned group. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-419) |
| F-420 | [x] Worktree sessions group by their parent repository. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-420) |
| F-421 | [x] Project collapse state persists per device. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-421) |
| F-422 | [x] The current session's project remains expanded despite a collapsed preference. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-422) |
| F-423 | [x] Collapsed projects show session count and working-session indicator. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-423) |
| F-424 | [x] Search temporarily expands matching project groups. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-424) |
| F-425 | [ ] Session filtering matches title and working folder without case sensitivity. | `antiquota/ui/app.js:246` | None identified | [Pending](TEST_REPORT.md#f-425) |
| F-426 | [x] No sessions and no title matches have distinct empty messages. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-426) |
| F-427 | [x] Searches of two or more characters query message contents after 220 ms. | `antiquota/ui/app.js:292` | None identified | [Passed](TEST_REPORT.md#f-427) |
| F-428 | [x] Older message-search responses cannot replace newer results. | `antiquota/ui/app.js:292` | None identified | [Passed](TEST_REPORT.md#f-428) |
| F-429 | [x] Search hits show title, author, snippet, time and folder tooltip. | `antiquota/ui/app.js:292` | None identified | [Passed](TEST_REPORT.md#f-429) |
| F-430 | [x] Show archived exposes archived sessions and Hide archived conceals them. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-430) |
| F-431 | [x] The current archived session stays visible while archives are hidden. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-431) |
| F-432 | [x] Session Rename opens the session and its title editor. | `antiquota/ui/app.js:306` | None identified | [Passed](TEST_REPORT.md#f-432) |
| F-433 | [x] Session Pin to top persists the pinned state. | `antiquota/ui/app.js:306` | None identified | [Passed](TEST_REPORT.md#f-433) |
| F-434 | [x] Session Unpin clears the pinned state. | `antiquota/ui/app.js:306` | None identified | [Passed](TEST_REPORT.md#f-434) |
| F-435 | [x] Session Fork creates and opens a session from handoff state. | `antiquota/ui/app.js:306` | None identified | [Passed](TEST_REPORT.md#f-435) |
| F-436 | [x] Session Archive persists archived state. | `antiquota/ui/app.js:306` | None identified | [Passed](TEST_REPORT.md#f-436) |
| F-437 | [x] Session Unarchive clears archived state. | `antiquota/ui/app.js:306` | None identified | [Passed](TEST_REPORT.md#f-437) |
| F-438 | [x] Session Delete requires confirmation and preserves project files. | `antiquota/ui/app.js:306` | None identified | [Passed](TEST_REPORT.md#f-438) |
| F-439 | [x] Project New session here preselects its folder without a worktree. | `antiquota/ui/app.js:329` | None identified | [Passed](TEST_REPORT.md#f-439) |
| F-440 | [x] Project New session in a git worktree preselects its folder with a worktree. | `antiquota/ui/app.js:329` | None identified | [Passed](TEST_REPORT.md#f-440) |
| F-441 | [x] Collapse every other project persists the other project folders as collapsed. | `antiquota/ui/app.js:329` | None identified | [Passed](TEST_REPORT.md#f-441) |
| F-442 | [x] Expand every project clears project collapse preferences. | `antiquota/ui/app.js:329` | None identified | [Passed](TEST_REPORT.md#f-442) |
| F-443 | [ ] Show in Finder / Explorer reveals the project folder in desktop mode. | `antiquota/ui/app.js:329` | None identified | [Pending](TEST_REPORT.md#f-443) |
| F-444 | [x] Copy path copies the complete project-folder path. | `antiquota/ui/app.js:329` | None identified | [Passed](TEST_REPORT.md#f-444) |
| F-445 | [x] Archive its sessions archives every listed unarchived session. | `antiquota/ui/app.js:329` | None identified | [Passed](TEST_REPORT.md#f-445) |
| F-446 | [x] Delete archived sessions confirms before deleting listed archived transcripts. | `antiquota/ui/app.js:329` | None identified | [Passed](TEST_REPORT.md#f-446) |
| F-447 | [x] Delete all sessions confirms before deleting every listed transcript. | `antiquota/ui/app.js:329` | None identified | [Passed](TEST_REPORT.md#f-447) |
| F-448 | [x] Session actions open from right-click and the More button. | `antiquota/ui/app.js:306` | None identified | [Passed](TEST_REPORT.md#f-448) |
| F-449 | [x] Project actions open from right-click and the Project actions button. | `antiquota/ui/app.js:329` | None identified | [Passed](TEST_REPORT.md#f-449) |
| F-450 | [x] Context menus keep their right and bottom edges inside the viewport. | `antiquota/ui/app.js:306` | None identified | [Passed](TEST_REPORT.md#f-450) |
| F-451 | [x] Clicking outside a context menu closes it. | `antiquota/ui/app.js:367` | None identified | [Passed](TEST_REPORT.md#f-451) |
| F-452 | [x] Opening a session replaces its transcript and focuses the composer. | `antiquota/ui/app.js:968` | None identified | [Passed](TEST_REPORT.md#f-452) |
| F-453 | [ ] Opening a session restores that session's user-prompt history. | `antiquota/ui/app.js:968` | None identified | [Pending](TEST_REPORT.md#f-453) |
| F-454 | [ ] A stale open-session response does not overwrite a newer selection. | `antiquota/ui/app.js:968` | None identified | [Pending](TEST_REPORT.md#f-454) |
| F-455 | [ ] Switching sessions clears unsent attachment chips. | `antiquota/ui/app.js:968` | None identified | [Pending](TEST_REPORT.md#f-455) |
| F-456 | [ ] Deleting the current session opens the next recent session or start screen. | `antiquota/ui/app.js:996` | None identified | [Pending](TEST_REPORT.md#f-456) |
| F-457 | [ ] An empty session offers up to three earlier sessions from the same folder. | `antiquota/ui/app.js:606` | None identified | [Pending](TEST_REPORT.md#f-457) |
| F-458 | [ ] Selecting an earlier session inherits its handoff state for the next prompt. | `antiquota/ui/app.js:606` | None identified | [Pending](TEST_REPORT.md#f-458) |
| F-459 | [ ] Start and empty-session screens offer sign-in when no accounts exist. | `antiquota/ui/app.js:584` | None identified | [Pending](TEST_REPORT.md#f-459) |
| F-460 | [ ] The empty-session screen offers Set a goal and explains its working folder. | `antiquota/ui/app.js:590` | None identified | [Pending](TEST_REPORT.md#f-460) |
| F-461 | [ ] The no-session start screen offers New session. | `antiquota/ui/app.js:616` | None identified | [Pending](TEST_REPORT.md#f-461) |

## Composer, models and header

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-462 | [ ] The header shows session title and shortened folder or worktree branch. | `antiquota/ui/app.js:416` | None identified | [Pending](TEST_REPORT.md#f-462) |
| F-463 | [ ] The header path tooltip exposes the complete folder or worktree origin/base. | `antiquota/ui/app.js:416` | None identified | [Pending](TEST_REPORT.md#f-463) |
| F-464 | [ ] The header shows working, approval, waiting, checking, verifying and sleeping states. | `antiquota/ui/app.js:416` | None identified | [Pending](TEST_REPORT.md#f-464) |
| F-465 | [ ] The header shows cumulative input/output tokens and queued-message count. | `antiquota/ui/app.js:416` | None identified | [Pending](TEST_REPORT.md#f-465) |
| F-466 | [ ] The page title marks working state and includes the session title. | `antiquota/ui/app.js:416` | None identified | [Pending](TEST_REPORT.md#f-466) |
| F-467 | [ ] An idle empty composer disables Send. | `antiquota/ui/app.js:445` | None identified | [Pending](TEST_REPORT.md#f-467) |
| F-468 | [ ] A busy empty composer exposes Stop. | `antiquota/ui/app.js:445` | None identified | [Pending](TEST_REPORT.md#f-468) |
| F-469 | [ ] A busy nonempty composer exposes Queue. | `antiquota/ui/app.js:445` | None identified | [Pending](TEST_REPORT.md#f-469) |
| F-470 | [ ] The composer grows to fit text up to 40 percent of window height. | `antiquota/ui/app.js:1095` | None identified | [Pending](TEST_REPORT.md#f-470) |
| F-471 | [ ] Sending posts text and completed attachment paths to the current session. | `antiquota/ui/app.js:1100` | None identified | [Pending](TEST_REPORT.md#f-471) |
| F-472 | [ ] Sending is blocked while an attachment is uploading. | `antiquota/ui/app.js:1100` | None identified | [Pending](TEST_REPORT.md#f-472) |
| F-473 | [ ] A failed send restores message text for retry. | `antiquota/ui/app.js:1100` | None identified | [Pending](TEST_REPORT.md#f-473) |
| F-474 | [ ] Text entered without a session is sent after creating a session. | `antiquota/ui/app.js:1100` | None identified | [Pending](TEST_REPORT.md#f-474) |
| F-475 | [ ] Attachment-only messages can be sent without text. | `antiquota/ui/app.js:1100` | None identified | [Pending](TEST_REPORT.md#f-475) |
| F-476 | [ ] Queued text explains mid-turn delivery at the next step. | `antiquota/ui/app.js:1100` | None identified | [Pending](TEST_REPORT.md#f-476) |
| F-477 | [ ] Queued attachments explain that delivery follows the current turn. | `antiquota/ui/app.js:1100` | None identified | [Pending](TEST_REPORT.md#f-477) |
| F-478 | [ ] A ! command runs in the project folder and supplies output to the next prompt. | `antiquota/ui/app.js:1123` | None identified | [Pending](TEST_REPORT.md#f-478) |
| F-479 | [ ] A # note appends to project GEMINI.md and reports the saved file. | `antiquota/ui/app.js:1123` | None identified | [Pending](TEST_REPORT.md#f-479) |
| F-480 | [ ] Shell prefixes change styling, hints and the action label to Run. | `antiquota/ui/app.js:445` | None identified | [Pending](TEST_REPORT.md#f-480) |
| F-481 | [ ] Note prefixes change styling, hints and the action label to Save. | `antiquota/ui/app.js:445` | None identified | [Pending](TEST_REPORT.md#f-481) |
| F-482 | [ ] Prompt history restores earlier messages and returns to the current draft. | `antiquota/ui/app.js:1386` | None identified | [Pending](TEST_REPORT.md#f-482) |
| F-483 | [ ] Models group into families and Gemini versus Claude/GPT quota pools. | `antiquota/ui/app.js:1045` | None identified | [Pending](TEST_REPORT.md#f-483) |
| F-484 | [ ] An unknown current model remains selectable. | `antiquota/ui/app.js:1045` | None identified | [Pending](TEST_REPORT.md#f-484) |
| F-485 | [ ] Families with one effort disable effort selection. | `antiquota/ui/app.js:1066` | None identified | [Pending](TEST_REPORT.md#f-485) |
| F-486 | [ ] Changing family preserves matching effort, then prefers high effort. | `antiquota/ui/app.js:1075` | None identified | [Pending](TEST_REPORT.md#f-486) |
| F-487 | [ ] Verifier choices expose concrete model IDs and effort labels. | `antiquota/ui/app.js:1082` | None identified | [Pending](TEST_REPORT.md#f-487) |
| F-488 | [ ] Changing the composer model family patches the session model. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-488) |
| F-489 | [ ] Changing the composer effort patches the session model. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-489) |
| F-490 | [ ] Changing the composer permission mode patches the session. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-490) |
| F-491 | [ ] Selecting bypass from the composer requires dangerous-action confirmation. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-491) |
| F-492 | [ ] Header rename saves a changed nonempty title on Enter. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-492) |
| F-493 | [ ] Header rename saves a changed nonempty title on blur. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-493) |
| F-494 | [ ] Escape cancels header rename. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-494) |
| F-495 | [ ] The meter shows context fraction with warm and hot thresholds. | `antiquota/ui/app.js:2486` | None identified | [Pending](TEST_REPORT.md#f-495) |
| F-496 | [ ] The meter shows cumulative input, output and thinking tokens. | `antiquota/ui/app.js:2486` | None identified | [Pending](TEST_REPORT.md#f-496) |
| F-497 | [ ] The meter shows serving account and its lowest-window quota in the selected pool. | `antiquota/ui/app.js:2486` | None identified | [Pending](TEST_REPORT.md#f-497) |
| F-498 | [ ] The meter shows the number of currently available accounts in the pool. | `antiquota/ui/app.js:2486` | None identified | [Pending](TEST_REPORT.md#f-498) |
| F-499 | [ ] Model context capacities are 200000 for Claude, 131072 for GPT-OSS and 1048576 otherwise. | `antiquota/ui/app.js:2484` | None identified | [Pending](TEST_REPORT.md#f-499) |

## Transcript rendering

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-500 | [ ] Model content is escaped before supported Markdown is inserted. | `antiquota/ui/app.js:142` | None identified | [Pending](TEST_REPORT.md#f-500) |
| F-501 | [ ] Markdown renders fenced code blocks. | `antiquota/ui/app.js:156` | None identified | [Pending](TEST_REPORT.md#f-501) |
| F-502 | [ ] Markdown renders four heading levels. | `antiquota/ui/app.js:156` | None identified | [Pending](TEST_REPORT.md#f-502) |
| F-503 | [ ] Markdown renders ordered and unordered lists. | `antiquota/ui/app.js:156` | None identified | [Pending](TEST_REPORT.md#f-503) |
| F-504 | [ ] Markdown renders blockquotes. | `antiquota/ui/app.js:156` | None identified | [Pending](TEST_REPORT.md#f-504) |
| F-505 | [ ] Markdown renders tables with a header separator. | `antiquota/ui/app.js:156` | None identified | [Pending](TEST_REPORT.md#f-505) |
| F-506 | [ ] Markdown renders paragraphs and inline bold, emphasis and code. | `antiquota/ui/app.js:156` | None identified | [Pending](TEST_REPORT.md#f-506) |
| F-507 | [ ] HTTP Markdown links open externally and unsupported schemes remain text. | `antiquota/ui/app.js:142` | None identified | [Pending](TEST_REPORT.md#f-507) |
| F-508 | [ ] File Markdown links open the file viewer. | `antiquota/ui/app.js:142` | None identified | [Pending](TEST_REPORT.md#f-508) |
| F-509 | [ ] Code-block Copy copies only the code contents. | `antiquota/ui/app.js:200` | None identified | [Pending](TEST_REPORT.md#f-509) |
| F-510 | [ ] Copy message copies the completed assistant reply. | `antiquota/ui/app.js:865` | None identified | [Pending](TEST_REPORT.md#f-510) |
| F-511 | [ ] User messages identify user, goal, nudge, job and loop origins. | `antiquota/ui/app.js:626` | None identified | [Pending](TEST_REPORT.md#f-511) |
| F-512 | [ ] Visible goal prompts omit repeated standing instructions but retain full tooltip text. | `antiquota/ui/app.js:626` | None identified | [Pending](TEST_REPORT.md#f-512) |
| F-513 | [ ] Steered messages are marked as arriving while the agent worked. | `antiquota/ui/app.js:626` | None identified | [Pending](TEST_REPORT.md#f-513) |
| F-514 | [ ] Turn markers show number, account, model, clock time and rewind. | `antiquota/ui/app.js:871` | None identified | [Pending](TEST_REPORT.md#f-514) |
| F-515 | [ ] Streaming deltas update one assistant node until its completed reply replaces them. | `antiquota/ui/app.js:871` | None identified | [Pending](TEST_REPORT.md#f-515) |
| F-516 | [ ] Completed tool rows replace running rows without losing expanded state. | `antiquota/ui/app.js:871` | None identified | [Pending](TEST_REPORT.md#f-516) |
| F-517 | [ ] Tool rows identify run, read, list, search, find, edit, web and MCP actions. | `antiquota/ui/app.js:636` | None identified | [Pending](TEST_REPORT.md#f-517) |
| F-518 | [ ] Unknown tool rows display a readable name and short string argument. | `antiquota/ui/app.js:636` | None identified | [Pending](TEST_REPORT.md#f-518) |
| F-519 | [ ] Tool rows show running, completed or error state with elapsed time. | `antiquota/ui/app.js:691` | None identified | [Pending](TEST_REPORT.md#f-519) |
| F-520 | [ ] Tool rows show added and deleted line counts for diffs. | `antiquota/ui/app.js:691` | None identified | [Pending](TEST_REPORT.md#f-520) |
| F-521 | [ ] ANSI terminal color sequences are removed from command output. | `antiquota/ui/app.js:691` | None identified | [Pending](TEST_REPORT.md#f-521) |
| F-522 | [ ] Command previews show the final four lines with an additional-line indicator. | `antiquota/ui/app.js:669` | None identified | [Pending](TEST_REPORT.md#f-522) |
| F-523 | [ ] Diff previews show up to six changed lines with an additional-line indicator. | `antiquota/ui/app.js:669` | None identified | [Pending](TEST_REPORT.md#f-523) |
| F-524 | [ ] Expanded tools display complete diff or output. | `antiquota/ui/app.js:683` | None identified | [Pending](TEST_REPORT.md#f-524) |
| F-525 | [ ] Expanded tools include extended errors or JSON parameters when output is absent. | `antiquota/ui/app.js:683` | None identified | [Pending](TEST_REPORT.md#f-525) |
| F-526 | [ ] Tool headers and previews toggle complete output expansion. | `antiquota/ui/app.js:691` | None identified | [Pending](TEST_REPORT.md#f-526) |
| F-527 | [ ] Grouped mode folds adjacent tool steps into one expandable summary. | `antiquota/ui/app.js:730` | None identified | [Pending](TEST_REPORT.md#f-527) |
| F-528 | [ ] Grouped summaries count unique read and edited paths. | `antiquota/ui/app.js:730` | None identified | [Pending](TEST_REPORT.md#f-528) |
| F-529 | [ ] Grouped summaries show additions, deletions, failures and step count. | `antiquota/ui/app.js:730` | None identified | [Pending](TEST_REPORT.md#f-529) |
| F-530 | [ ] Grouped summaries show the active running tool. | `antiquota/ui/app.js:730` | None identified | [Pending](TEST_REPORT.md#f-530) |
| F-531 | [ ] Thinking blocks display reasoning Markdown and token counts when present. | `antiquota/ui/app.js:767` | None identified | [Pending](TEST_REPORT.md#f-531) |
| F-532 | [ ] Short thinking blocks start expanded in detailed mode. | `antiquota/ui/app.js:767` | None identified | [Pending](TEST_REPORT.md#f-532) |
| F-533 | [ ] Compact mode hides previews and changes thinking expansion. | `antiquota/ui/app.js:2470` | None identified | [Pending](TEST_REPORT.md#f-533) |
| F-534 | [ ] Detailed or compact mode persists per device. | `antiquota/ui/app.js:2470` | None identified | [Pending](TEST_REPORT.md#f-534) |
| F-535 | [ ] Expand all controls tool groups, individual output and thinking blocks. | `antiquota/ui/app.js:2478` | None identified | [Pending](TEST_REPORT.md#f-535) |
| F-536 | [ ] Turn completion shows status, duration, input/output use and serving account. | `antiquota/ui/app.js:838` | None identified | [Pending](TEST_REPORT.md#f-536) |
| F-537 | [ ] Completed and failed turns remove live transcript markers. | `antiquota/ui/app.js:871` | None identified | [Pending](TEST_REPORT.md#f-537) |
| F-538 | [ ] Events auto-scroll only within 120 pixels of the transcript bottom. | `antiquota/ui/app.js:576` | None identified | [Pending](TEST_REPORT.md#f-538) |
| F-539 | [ ] Handoff notices identify the transition to another account. | `antiquota/ui/app.js:785` | None identified | [Pending](TEST_REPORT.md#f-539) |
| F-540 | [ ] Quota-wall and no-account notices have distinct actionable presentations. | `antiquota/ui/app.js:785` | None identified | [Pending](TEST_REPORT.md#f-540) |
| F-541 | [ ] Goal notices indicate whether the objective is met. | `antiquota/ui/app.js:785` | None identified | [Pending](TEST_REPORT.md#f-541) |
| F-542 | [ ] Check notices show pass/fail and expandable output. | `antiquota/ui/app.js:785` | None identified | [Pending](TEST_REPORT.md#f-542) |
| F-543 | [ ] Verifier notices show done, not met or indeterminate state. | `antiquota/ui/app.js:785` | None identified | [Pending](TEST_REPORT.md#f-543) |
| F-544 | [ ] Verifier notices show missing work and verifier account/model. | `antiquota/ui/app.js:785` | None identified | [Pending](TEST_REPORT.md#f-544) |
| F-545 | [ ] Errors display their error text without markup interpretation. | `antiquota/ui/app.js:785` | None identified | [Pending](TEST_REPORT.md#f-545) |
| F-546 | [ ] Denied-command notices offer adding the suggested rules. | `antiquota/ui/app.js:785` | None identified | [Pending](TEST_REPORT.md#f-546) |
| F-547 | [ ] Allow and retry grants suggested command rules and sends a retry prompt. | `antiquota/ui/app.js:785` | None identified | [Pending](TEST_REPORT.md#f-547) |
| F-548 | [ ] Denied edits without command rules offer accept edits and retry. | `antiquota/ui/app.js:785` | None identified | [Pending](TEST_REPORT.md#f-548) |
| F-549 | [ ] Rewind requires a specified turn and confirmation before restoring files. | `antiquota/ui/app.js:855` | None identified | [Pending](TEST_REPORT.md#f-549) |
| F-550 | [ ] Rewind notices identify files changed outside the agent after its edit. | `antiquota/ui/app.js:785` | None identified | [Pending](TEST_REPORT.md#f-550) |
| F-551 | [ ] Markdown export includes prompts, replies, reasoning, tools, turns and notices. | `antiquota/ui/app.js:1414` | None identified | [Pending](TEST_REPORT.md#f-551) |
| F-552 | [ ] Export uses a sanitized title limited to sixty characters plus .md. | `antiquota/ui/app.js:1414` | None identified | [Pending](TEST_REPORT.md#f-552) |

## Goals, plans, jobs and approvals

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-553 | [ ] The goal dialog reuses an existing goal or composer objective. | `antiquota/ui/app.js:1970` | None identified | [Pending](TEST_REPORT.md#f-553) |
| F-554 | [ ] Goal controls expose objective, optional check, verifier, model and round limit. | `antiquota/ui/app.js:1970` | None identified | [Pending](TEST_REPORT.md#f-554) |
| F-555 | [ ] Start goal posts all selected controls. | `antiquota/ui/app.js:1981` | None identified | [Pending](TEST_REPORT.md#f-555) |
| F-556 | [ ] Starting an identical composer objective clears that composer text. | `antiquota/ui/app.js:1981` | None identified | [Pending](TEST_REPORT.md#f-556) |
| F-557 | [ ] Goal bars show objective, state/rounds, check and verifier model. | `antiquota/ui/app.js:462` | None identified | [Pending](TEST_REPORT.md#f-557) |
| F-558 | [ ] Goal Pause stops an active session. | `antiquota/ui/app.js:462` | None identified | [Pending](TEST_REPORT.md#f-558) |
| F-559 | [ ] Goal Resume restarts an unmet inactive goal. | `antiquota/ui/app.js:462` | None identified | [Pending](TEST_REPORT.md#f-559) |
| F-560 | [ ] Goal Edit opens the existing goal dialog. | `antiquota/ui/app.js:462` | None identified | [Pending](TEST_REPORT.md#f-560) |
| F-561 | [ ] Goal Remove deletes its goal. | `antiquota/ui/app.js:462` | None identified | [Pending](TEST_REPORT.md#f-561) |
| F-562 | [ ] A successful completed idle plan-mode turn exposes plan-ready controls. | `antiquota/ui/app.js:486` | None identified | [Pending](TEST_REPORT.md#f-562) |
| F-563 | [ ] Execute plan switches to accept-edits before sending its execution instruction. | `antiquota/ui/app.js:486` | None identified | [Pending](TEST_REPORT.md#f-563) |
| F-564 | [ ] Execute with approvals switches to default before sending its execution instruction. | `antiquota/ui/app.js:486` | None identified | [Pending](TEST_REPORT.md#f-564) |
| F-565 | [ ] Revise focuses the composer with a revision placeholder. | `antiquota/ui/app.js:486` | None identified | [Pending](TEST_REPORT.md#f-565) |
| F-566 | [ ] Running jobs show command, elapsed time, check-in and agent attribution. | `antiquota/ui/app.js:507` | None identified | [Pending](TEST_REPORT.md#f-566) |
| F-567 | [ ] Job Log opens that job's output. | `antiquota/ui/app.js:507` | None identified | [Pending](TEST_REPORT.md#f-567) |
| F-568 | [ ] Job Stop requests ending that job. | `antiquota/ui/app.js:507` | None identified | [Pending](TEST_REPORT.md#f-568) |
| F-569 | [ ] Job logs show no-output text when empty and scroll to the latest output. | `antiquota/ui/app.js:528` | None identified | [Pending](TEST_REPORT.md#f-569) |
| F-570 | [ ] Job events distinguish started, stopped, success, failure and unknown exit code. | `antiquota/ui/app.js:565` | None identified | [Pending](TEST_REPORT.md#f-570) |
| F-571 | [ ] Loop bars show prompt, interval and next run. | `antiquota/ui/app.js:507` | None identified | [Pending](TEST_REPORT.md#f-571) |
| F-572 | [ ] Daily schedule bars show prompt, daily time and next run. | `antiquota/ui/app.js:507` | None identified | [Pending](TEST_REPORT.md#f-572) |
| F-573 | [ ] Loop Stop deletes the configured loop. | `antiquota/ui/app.js:507` | None identified | [Pending](TEST_REPORT.md#f-573) |
| F-574 | [ ] Schedule Remove deletes the configured schedule. | `antiquota/ui/app.js:507` | None identified | [Pending](TEST_REPORT.md#f-574) |
| F-575 | [ ] Pending approval cards show command or edit target, preview and wait duration. | `antiquota/ui/app.js:537` | None identified | [Pending](TEST_REPORT.md#f-575) |
| F-576 | [ ] Allow once answers without persisting a rule. | `antiquota/ui/app.js:537` | None identified | [Pending](TEST_REPORT.md#f-576) |
| F-577 | [ ] Always allow answers with the suggested permanent command rule. | `antiquota/ui/app.js:537` | None identified | [Pending](TEST_REPORT.md#f-577) |
| F-578 | [ ] Allow all edits switches the session's edit permission behavior. | `antiquota/ui/app.js:537` | None identified | [Pending](TEST_REPORT.md#f-578) |
| F-579 | [ ] Decline sends a denial. | `antiquota/ui/app.js:537` | None identified | [Pending](TEST_REPORT.md#f-579) |
| F-580 | [ ] Answered cards show allow/decline, once/permanent state and reason. | `antiquota/ui/app.js:537` | None identified | [Pending](TEST_REPORT.md#f-580) |
| F-581 | [ ] Missing approval answers display the expired-request explanation. | `antiquota/ui/app.js:537` | None identified | [Pending](TEST_REPORT.md#f-581) |
| F-582 | [ ] A new pending approval requests native attention and an approval notification. | `antiquota/ui/app.js:871` | None identified | [Pending](TEST_REPORT.md#f-582) |

## Files and Git review

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-583 | [ ] The Files tab lists the selected session's folders and files. | `antiquota/ui/app.js:1545` | None identified | [Pending](TEST_REPORT.md#f-583) |
| F-584 | [ ] Breadcrumbs navigate to earlier directories. | `antiquota/ui/app.js:1545` | None identified | [Pending](TEST_REPORT.md#f-584) |
| F-585 | [ ] The parent row navigates to the parent directory. | `antiquota/ui/app.js:1545` | None identified | [Pending](TEST_REPORT.md#f-585) |
| F-586 | [ ] File rows display size and dimmed entries. | `antiquota/ui/app.js:1545` | None identified | [Pending](TEST_REPORT.md#f-586) |
| F-587 | [ ] An empty directory shows Empty folder. | `antiquota/ui/app.js:1545` | None identified | [Pending](TEST_REPORT.md#f-587) |
| F-588 | [ ] Mention inserts a file's @ path into the composer. | `antiquota/ui/app.js:1545` | None identified | [Pending](TEST_REPORT.md#f-588) |
| F-589 | [ ] Desktop Open launches a file's default application. | `antiquota/ui/app.js:1545` | None identified | [Pending](TEST_REPORT.md#f-589) |
| F-590 | [ ] Desktop Reveal opens Finder / Explorer for a file or directory. | `antiquota/ui/app.js:1545` | None identified | [Pending](TEST_REPORT.md#f-590) |
| F-591 | [ ] File links open the numbered file viewer. | `antiquota/ui/app.js:1219` | None identified | [Pending](TEST_REPORT.md#f-591) |
| F-592 | [ ] The viewer shows relative path and full-path tooltip. | `antiquota/ui/app.js:1224` | None identified | [Pending](TEST_REPORT.md#f-592) |
| F-593 | [ ] Line or line-range links highlight and center those lines. | `antiquota/ui/app.js:1224` | None identified | [Pending](TEST_REPORT.md#f-593) |
| F-594 | [ ] Viewer Copy copies complete contents. | `antiquota/ui/app.js:1224` | None identified | [Pending](TEST_REPORT.md#f-594) |
| F-595 | [ ] Viewer Close dismisses the viewer. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-595) |
| F-596 | [ ] Clicking the viewer backdrop dismisses the viewer. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-596) |
| F-597 | [ ] Files and Terminal tabs explain that a session is required when none is open. | `antiquota/ui/app.js:1895` | None identified | [Pending](TEST_REPORT.md#f-597) |
| F-598 | [ ] Git review shows branch and repository or worktree base/ahead/merged state. | `antiquota/ui/app.js:1573` | None identified | [Pending](TEST_REPORT.md#f-598) |
| F-599 | [ ] Uncommitted file rows show status, additions and deletions. | `antiquota/ui/app.js:1573` | None identified | [Pending](TEST_REPORT.md#f-599) |
| F-600 | [ ] Expanding uncommitted files lazily requests their diff. | `antiquota/ui/app.js:1573` | None identified | [Pending](TEST_REPORT.md#f-600) |
| F-601 | [ ] Expanding worktree committed files lazily requests the base-relative diff. | `antiquota/ui/app.js:1573` | None identified | [Pending](TEST_REPORT.md#f-601) |
| F-602 | [ ] Discard confirms deletion of a new file or loss of uncommitted edits. | `antiquota/ui/app.js:1573` | None identified | [Pending](TEST_REPORT.md#f-602) |
| F-603 | [ ] Commit all sends the message and refreshes Git review. | `antiquota/ui/app.js:1573` | None identified | [Pending](TEST_REPORT.md#f-603) |
| F-604 | [ ] The initial commit message uses the selected session title. | `antiquota/ui/app.js:1573` | None identified | [Pending](TEST_REPORT.md#f-604) |
| F-605 | [ ] Worktree review lists files committed relative to its base. | `antiquota/ui/app.js:1573` | None identified | [Pending](TEST_REPORT.md#f-605) |
| F-606 | [ ] Merge is enabled only for an unmerged worktree with commits ahead. | `antiquota/ui/app.js:1573` | None identified | [Pending](TEST_REPORT.md#f-606) |
| F-607 | [ ] Remove worktree confirms removal, archives the session and keeps its branch. | `antiquota/ui/app.js:1573` | None identified | [Pending](TEST_REPORT.md#f-607) |
| F-608 | [ ] Agent edit history groups recorded diffs by file, newest first. | `antiquota/ui/app.js:1895` | None identified | [Pending](TEST_REPORT.md#f-608) |
| F-609 | [ ] Expanding an edit-history file lazily displays all recorded diffs. | `antiquota/ui/app.js:1895` | None identified | [Pending](TEST_REPORT.md#f-609) |
| F-610 | [ ] Changes displays distinct no-session, no-edits and no-uncommitted messages. | `antiquota/ui/app.js:1895` | None identified | [Pending](TEST_REPORT.md#f-610) |

## Split, drawer and terminal

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-611 | [x] Split opens a second independently selected session. | `antiquota/ui/app.js:1626` | None identified | [Passed](TEST_REPORT.md#f-611) |
| F-612 | [x] Split selection persists and restores on reload when still valid. | `antiquota/ui/app.js:1626` | None identified | [Passed](TEST_REPORT.md#f-612) |
| F-613 | [x] Opening split hides the side panel. | `antiquota/ui/app.js:1626` | None identified | [Passed](TEST_REPORT.md#f-613) |
| F-614 | [x] Split choices list up to fourteen recent unarchived other sessions. | `antiquota/ui/app.js:1644` | None identified | [Passed](TEST_REPORT.md#f-614) |
| F-615 | [x] Split choices identify busy sessions and no-other-session state. | `antiquota/ui/app.js:1644` | None identified | [Passed](TEST_REPORT.md#f-615) |
| F-616 | [x] Close split removes the second session and persisted selection. | `antiquota/ui/app.js:1637` | None identified | [Passed](TEST_REPORT.md#f-616) |
| F-617 | [x] Swap sides exchanges the left and right sessions. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-617) |
| F-618 | [x] The embedded Close side by side action closes split view. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-618) |
| F-619 | [x] Cross-origin split control messages are ignored. | `antiquota/ui/app.js:1661` | None identified | [Passed](TEST_REPORT.md#f-619) |
| F-620 | [x] Embedded views omit sidebar, drawer and nested split controls. | `antiquota/ui/app.js:17` | None identified | [Passed](TEST_REPORT.md#f-620) |
| F-621 | [ ] Side-panel visibility persists per device. | `antiquota/ui/app.js:1671` | None identified | [Pending](TEST_REPORT.md#f-621) |
| F-622 | [ ] The Changes, Files or Terminal tab selection persists per device. | `antiquota/ui/app.js:1671` | None identified | [Pending](TEST_REPORT.md#f-622) |
| F-623 | [ ] Open side-panel content follows session selection. | `antiquota/ui/app.js:968` | None identified | [Pending](TEST_REPORT.md#f-623) |
| F-624 | [ ] Terminal assets are loaded lazily once. | `antiquota/ui/app.js:1820` | None identified | [Pending](TEST_REPORT.md#f-624) |
| F-625 | [ ] A terminal asset failure displays a useful load error. | `antiquota/ui/app.js:1833` | None identified | [Pending](TEST_REPORT.md#f-625) |
| F-626 | [ ] Each session retains an independent xterm instance across tab/session switches. | `antiquota/ui/app.js:1833` | None identified | [Pending](TEST_REPORT.md#f-626) |
| F-627 | [ ] Terminal starts a shell in the session folder and shows shell/folder in its title. | `antiquota/ui/app.js:1833` | None identified | [Pending](TEST_REPORT.md#f-627) |
| F-628 | [ ] Terminal keyboard input is posted to the engine. | `antiquota/ui/app.js:1833` | None identified | [Pending](TEST_REPORT.md#f-628) |
| F-629 | [ ] Streamed base64 bytes render as terminal output. | `antiquota/ui/app.js:1833` | None identified | [Pending](TEST_REPORT.md#f-629) |
| F-630 | [ ] First replayed output resets the terminal display. | `antiquota/ui/app.js:1833` | None identified | [Pending](TEST_REPORT.md#f-630) |
| F-631 | [ ] Resize fits xterm and posts dimensions after an 80 ms debounce. | `antiquota/ui/app.js:1833` | None identified | [Pending](TEST_REPORT.md#f-631) |
| F-632 | [ ] Clear clears the display without restarting the shell. | `antiquota/ui/app.js:1833` | None identified | [Pending](TEST_REPORT.md#f-632) |
| F-633 | [ ] Restart requests a fresh shell and reconnects output. | `antiquota/ui/app.js:1833` | None identified | [Pending](TEST_REPORT.md#f-633) |
| F-634 | [ ] Kill ends the shell. | `antiquota/ui/app.js:1833` | None identified | [Pending](TEST_REPORT.md#f-634) |
| F-635 | [ ] Exit marks the title exited and closes the output stream. | `antiquota/ui/app.js:1833` | None identified | [Pending](TEST_REPORT.md#f-635) |
| F-636 | [ ] Terminal input and startup errors render in red inside xterm. | `antiquota/ui/app.js:1833` | None identified | [Pending](TEST_REPORT.md#f-636) |
| F-637 | [ ] Opening or revisiting the Terminal tab focuses it. | `antiquota/ui/app.js:1833` | None identified | [Pending](TEST_REPORT.md#f-637) |
| F-638 | [ ] Terminal scrollback retains 5000 lines. | `antiquota/ui/app.js:1833` | None identified | [Pending](TEST_REPORT.md#f-638) |

## Attachments and dictation

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-639 | [ ] The picker uploads multiple files for the selected session. | `antiquota/ui/app.js:1351` | None identified | [Pending](TEST_REPORT.md#f-639) |
| F-640 | [ ] Pasted screenshot files upload with generic image.png renamed by timestamp. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-640) |
| F-641 | [ ] Dragging files into the window displays the drop overlay. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-641) |
| F-642 | [ ] Dropping files hides the overlay and attaches those files. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-642) |
| F-643 | [ ] Nested drag tracking keeps the overlay until every file drag leaves. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-643) |
| F-644 | [ ] Attaching without a session requests opening or creating one. | `antiquota/ui/app.js:1351` | None identified | [Pending](TEST_REPORT.md#f-644) |
| F-645 | [ ] Files over 25 MB are rejected by filename. | `antiquota/ui/app.js:1351` | None identified | [Pending](TEST_REPORT.md#f-645) |
| F-646 | [ ] Chips display thumbnail or type, filename, size and upload state. | `antiquota/ui/app.js:1341` | None identified | [Pending](TEST_REPORT.md#f-646) |
| F-647 | [ ] Remove excludes an unsent attachment from the next message. | `antiquota/ui/app.js:1341` | None identified | [Pending](TEST_REPORT.md#f-647) |
| F-648 | [ ] Failed uploads remove their chip and display the error. | `antiquota/ui/app.js:1351` | None identified | [Pending](TEST_REPORT.md#f-648) |
| F-649 | [ ] Sent image attachments render as authenticated clickable thumbnails. | `antiquota/ui/app.js:1377` | None identified | [Pending](TEST_REPORT.md#f-649) |
| F-650 | [ ] Sent non-image attachments render as filename/type chips. | `antiquota/ui/app.js:1377` | None identified | [Pending](TEST_REPORT.md#f-650) |
| F-651 | [ ] Dictation starts from idle and stops from listening or recording. | `antiquota/ui/app.js:1266` | None identified | [Pending](TEST_REPORT.md#f-651) |
| F-652 | [ ] Dictation cannot restart while transcription is running. | `antiquota/ui/app.js:1266` | None identified | [Pending](TEST_REPORT.md#f-652) |
| F-653 | [ ] The mic and status label indicate listening, recording and transcribing. | `antiquota/ui/app.js:1245` | None identified | [Pending](TEST_REPORT.md#f-653) |
| F-654 | [ ] Recording status shows elapsed time and an Escape-cancel hint. | `antiquota/ui/app.js:1245` | None identified | [Pending](TEST_REPORT.md#f-654) |
| F-655 | [ ] Browser recognition uses continuous interim results in navigator language. | `antiquota/ui/app.js:1272` | None identified | [Pending](TEST_REPORT.md#f-655) |
| F-656 | [ ] Recognition appends recognized text after current composer content. | `antiquota/ui/app.js:1272` | None identified | [Pending](TEST_REPORT.md#f-656) |
| F-657 | [ ] Permission, service and network errors fall back to audio recording. | `antiquota/ui/app.js:1272` | None identified | [Pending](TEST_REPORT.md#f-657) |
| F-658 | [ ] Aborted or no-speech recognition ends without an error toast. | `antiquota/ui/app.js:1272` | None identified | [Pending](TEST_REPORT.md#f-658) |
| F-659 | [ ] Capture requests mono microphone input with echo cancellation and noise suppression. | `antiquota/ui/app.js:1286` | None identified | [Pending](TEST_REPORT.md#f-659) |
| F-660 | [ ] Microphone access failure displays its error. | `antiquota/ui/app.js:1286` | None identified | [Pending](TEST_REPORT.md#f-660) |
| F-661 | [ ] Stopping or canceling disconnects audio nodes and stops microphone tracks. | `antiquota/ui/app.js:1297` | None identified | [Pending](TEST_REPORT.md#f-661) |
| F-662 | [ ] Captures shorter than half a second are rejected before upload. | `antiquota/ui/app.js:1297` | None identified | [Pending](TEST_REPORT.md#f-662) |
| F-663 | [ ] Peak amplitude below 0.01 is rejected as silence before upload. | `antiquota/ui/app.js:1297` | None identified | [Pending](TEST_REPORT.md#f-663) |
| F-664 | [ ] Legacy recording uploads 16000 Hz mono PCM WAV. Authorized replacement: use MediaRecorder directly and upload a supported recorded container with the correct X-Audio-Type; preserve recorded-WAV input tests and the client/server silence guard. | `antiquota/ui/app.js:1319` | None identified | [Pending](TEST_REPORT.md#f-664) |
| F-665 | [ ] Empty transcription reports Nothing heard without inserting text. | `antiquota/ui/app.js:1297` | None identified | [Pending](TEST_REPORT.md#f-665) |
| F-666 | [ ] Transcription inserts at the selection and restores composer focus. | `antiquota/ui/app.js:1258` | None identified | [Pending](TEST_REPORT.md#f-666) |
| F-667 | [ ] Escape cancels recording. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-667) |
| F-668 | [ ] Escape aborts live speech recognition. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-668) |

## Settings sections

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-669 | [ ] General settings exposes new-session defaults, keep-awake, autostart and notification controls. | `antiquota/ui/app.js:2098` | None identified | [Pending](TEST_REPORT.md#f-669) |
| F-670 | [ ] Appearance settings exposes desktop size and transcript presentation controls. | `antiquota/ui/app.js:2098` | None identified | [Pending](TEST_REPORT.md#f-670) |
| F-671 | [x] Keyboard settings exposes shortcuts and slash-command reference. | `antiquota/ui/app.js:2098` | None identified | [Passed](TEST_REPORT.md#f-671) |
| F-672 | [x] Accounts settings exposes accounts, sign-in, rotation controls and routing history. | `antiquota/ui/app.js:2098` | None identified | [Passed](TEST_REPORT.md#f-672) |
| F-673 | [x] Usage settings exposes period controls, statistics, chart and account/model breakdowns. | `antiquota/ui/app.js:2098` | None identified | [Passed](TEST_REPORT.md#f-673) |
| F-674 | [x] Agents & goals settings exposes verifier, round, approval and denial controls. | `antiquota/ui/app.js:2098` | None identified | [Passed](TEST_REPORT.md#f-674) |
| F-675 | [ ] Permissions settings exposes independent allow and deny rules. | `antiquota/ui/app.js:2098` | None identified | [Pending](TEST_REPORT.md#f-675) |
| F-676 | [x] Rules settings exposes private GEMINI.md and triggered rule files. | `antiquota/ui/app.js:2098` | None identified | [Passed](TEST_REPORT.md#f-676) |
| F-677 | [x] Skills settings exposes installed and source skill comparisons and private editors. | `antiquota/ui/app.js:2098` | None identified | [Passed](TEST_REPORT.md#f-677) |
| F-678 | [x] Hooks settings exposes event hook entries and enable/edit/copy/delete controls. | `antiquota/ui/app.js:2098` | None identified | [Passed](TEST_REPORT.md#f-678) |
| F-679 | [x] MCP servers settings exposes tool servers and their configuration controls. | `antiquota/ui/app.js:2098` | None identified | [Passed](TEST_REPORT.md#f-679) |
| F-680 | [x] Plugins settings exposes installed bundles and copying differing CLI bundles. | `antiquota/ui/app.js:2098` | None identified | [Passed](TEST_REPORT.md#f-680) |
| F-681 | [ ] Engine settings exposes diagnostics, log, data directory, restart and private-stack replacement. | `antiquota/ui/app.js:2098` | None identified | [Pending](TEST_REPORT.md#f-681) |
| F-682 | [x] Settings immediately displays cached content or a loading message while reading fresh data. | `antiquota/ui/app.js:2115` | None identified | [Passed](TEST_REPORT.md#f-682) |
| F-683 | [x] Fresh data does not redraw an active editor or focused settings input. | `antiquota/ui/app.js:2124` | None identified | [Passed](TEST_REPORT.md#f-683) |
| F-684 | [ ] Navigation shows installed item counts, account attention and private stack path. | `antiquota/ui/app.js:2131` | None identified | [Pending](TEST_REPORT.md#f-684) |
| F-685 | [x] Close settings hides the panel and closes the active editor. | `antiquota/ui/app.js:2123` | None identified | [Passed](TEST_REPORT.md#f-685) |

## General and appearance controls

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-686 | [x] Default model family saves the selected concrete model and effort. | `antiquota/ui/app.js:2251` | None identified | [Passed](TEST_REPORT.md#f-686) |
| F-687 | [x] Default effort saves the chosen model variant. | `antiquota/ui/app.js:2251` | None identified | [Passed](TEST_REPORT.md#f-687) |
| F-688 | [x] Default mode persists default, accept-edits, plan or bypass. | `antiquota/ui/app.js:2251` | None identified | [Passed](TEST_REPORT.md#f-688) |
| F-689 | [x] Default folder persists the entered folder and falls back to home when empty. | `antiquota/ui/app.js:2251` | None identified | [Passed](TEST_REPORT.md#f-689) |
| F-690 | [x] Separate git worktree by default persists on/off. | `antiquota/ui/app.js:2251` | None identified | [Passed](TEST_REPORT.md#f-690) |
| F-691 | [x] Keep awake while agents work persists on/off. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-691) |
| F-692 | [ ] Open at login reads current native autostart state. | `antiquota/ui/app.js:2226` | None identified | [Pending](TEST_REPORT.md#f-692) |
| F-693 | [ ] Open at login enables or disables native autostart. | `antiquota/ui/app.js:2226` | None identified | [Pending](TEST_REPORT.md#f-693) |
| F-694 | [ ] An unavailable native autostart adapter removes its settings row. | `antiquota/ui/app.js:2226` | None identified | [Pending](TEST_REPORT.md#f-694) |
| F-695 | [x] Turn-finished notifications can be enabled or disabled independently. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-695) |
| F-696 | [x] Goal-met notifications can be enabled or disabled independently. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-696) |
| F-697 | [x] Approval-needed notifications can be enabled or disabled independently. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-697) |
| F-698 | [x] Job-finished notifications can be enabled or disabled independently. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-698) |
| F-699 | [x] Exhausted-quota notifications can be enabled or disabled independently. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-699) |
| F-700 | [x] Also while antiquota is in front persists focused-window notification preference. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-700) |
| F-701 | [x] Focused visible app windows suppress notifications unless their focused preference is on. | `antiquota/ui/app.js:1398` | None identified | [Passed](TEST_REPORT.md#f-701) |
| F-702 | [ ] Native notification permission is requested before sending an allowed notification. | `antiquota/ui/app.js:1398` | None identified | [Pending](TEST_REPORT.md#f-702) |
| F-703 | [x] Browser notifications require granted permission and tolerate notification failures. | `antiquota/ui/app.js:1398` | None identified | [Passed](TEST_REPORT.md#f-703) |
| F-704 | [ ] The first browser message requests notification permission when its state is default. | `antiquota/ui/app.js:1100` | None identified | [Pending](TEST_REPORT.md#f-704) |
| F-705 | [ ] Native Interface size offers 80, 90, 100, 110, 125 and 140 percent presets. | `antiquota/ui/app.js:2318` | None identified | [Pending](TEST_REPORT.md#f-705) |
| F-706 | [x] Browser Interface size explains the browser zoom controls instead of applying CSS scale. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-706) |
| F-707 | [x] Grouped steps saves grouped mode and rebuilds the selected transcript. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-707) |
| F-708 | [x] Every step saves ungrouped mode and rebuilds the selected transcript. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-708) |
| F-709 | [x] Previews and thinking selects detailed or compact transcript mode. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-709) |
| F-710 | [x] Times on turns persists timestamp visibility. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-710) |

## Account cards, rotation and usage

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-711 | [x] Accounts Probe now probes every account. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-711) |
| F-712 | [x] Accounts Add account opens sign-in. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-712) |
| F-713 | [x] Account cards show identity and paused, busy and last-probed states. | `antiquota/ui/app.js:1773` | None identified | [Passed](TEST_REPORT.md#f-713) |
| F-714 | [x] Account cards show today's input, output, thinking and turn counts. | `antiquota/ui/app.js:1773` | None identified | [Passed](TEST_REPORT.md#f-714) |
| F-715 | [x] Gemini and Claude/GPT quota windows appear separately. | `antiquota/ui/app.js:1773` | None identified | [Passed](TEST_REPORT.md#f-715) |
| F-716 | [x] Quota windows show their reset time or disabled-window explanation in a tooltip. | `antiquota/ui/app.js:1773` | None identified | [Passed](TEST_REPORT.md#f-716) |
| F-717 | [x] Account cards show walls, signed-out, cooldown and verification reasons. | `antiquota/ui/app.js:1773` | None identified | [Passed](TEST_REPORT.md#f-717) |
| F-718 | [ ] Pause keeps an account out of rotation. | `antiquota/ui/app.js:1773` | None identified | [Pending](TEST_REPORT.md#f-718) |
| F-719 | [ ] Resume puts a paused account back into rotation. | `antiquota/ui/app.js:1773` | None identified | [Pending](TEST_REPORT.md#f-719) |
| F-720 | [x] Check refreshes quota and sign-in state. | `antiquota/ui/app.js:1773` | None identified | [Passed](TEST_REPORT.md#f-720) |
| F-721 | [x] Sign in again opens the account sign-in dialog. | `antiquota/ui/app.js:1773` | None identified | [Passed](TEST_REPORT.md#f-721) |
| F-722 | [ ] Remove requires confirmation and preserves separate CLI sign-in. | `antiquota/ui/app.js:1773` | None identified | [Pending](TEST_REPORT.md#f-722) |
| F-723 | [x] A verification-required account opens its exact Google verification URL. | `antiquota/ui/app.js:1687` | None identified | [Passed](TEST_REPORT.md#f-723) |
| F-724 | [x] Check again reports readiness or continued verification after probing. | `antiquota/ui/app.js:1687` | None identified | [Passed](TEST_REPORT.md#f-724) |
| F-725 | [x] Next account selects soonest-reset-first, one-by-one or round-robin rotation. | `antiquota/ui/app.js:2273` | None identified | [Passed](TEST_REPORT.md#f-725) |
| F-726 | [x] Reserve accepts and persists 0–50 percent. | `antiquota/ui/app.js:2273` | None identified | [Passed](TEST_REPORT.md#f-726) |
| F-727 | [x] Quota check accepts and persists 2–240 minutes. | `antiquota/ui/app.js:2273` | None identified | [Passed](TEST_REPORT.md#f-727) |
| F-728 | [x] Routing history displays newest-first pick decisions with session and prior account. | `antiquota/ui/app.js:1799` | None identified | [Passed](TEST_REPORT.md#f-728) |
| F-729 | [x] Routing history displays walls, cooldowns, retries, signed-out, paused and resumed decisions. | `antiquota/ui/app.js:1799` | None identified | [Passed](TEST_REPORT.md#f-729) |
| F-730 | [x] No routing decisions has an explicit empty state. | `antiquota/ui/app.js:1799` | None identified | [Passed](TEST_REPORT.md#f-730) |
| F-731 | [x] Usage selects seven, fourteen or thirty days. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-731) |
| F-732 | [x] Usage displays period total tokens and turns. | `antiquota/ui/app.js:1708` | None identified | [Passed](TEST_REPORT.md#f-732) |
| F-733 | [x] Usage displays today's tokens and turns. | `antiquota/ui/app.js:1708` | None identified | [Passed](TEST_REPORT.md#f-733) |
| F-734 | [x] Usage displays peak day and mean tokens per turn. | `antiquota/ui/app.js:1708` | None identified | [Passed](TEST_REPORT.md#f-734) |
| F-735 | [x] Daily chart bars highlight the latest day with the accent color. | `antiquota/ui/app.js:1708` | None identified | [Passed](TEST_REPORT.md#f-735) |
| F-736 | [x] Hover shows date, tokens, turns and account contributions sorted by tokens. | `antiquota/ui/app.js:1708` | None identified | [Passed](TEST_REPORT.md#f-736) |
| F-737 | [x] Chart tooltips stay inside left, middle and right chart boundaries. | `antiquota/ui/app.js:1708` | None identified | [Passed](TEST_REPORT.md#f-737) |
| F-738 | [x] Usage account tables show period tokens, today tokens and turns ordered by total tokens. | `antiquota/ui/app.js:1756` | None identified | [Passed](TEST_REPORT.md#f-738) |
| F-739 | [x] Usage model tables show tokens and percentage share ordered by tokens. | `antiquota/ui/app.js:1762` | None identified | [Passed](TEST_REPORT.md#f-739) |
| F-740 | [x] Empty periods display No turns in this period yet. | `antiquota/ui/app.js:1759` | None identified | [Passed](TEST_REPORT.md#f-740) |
| F-741 | [x] The sidebar account pill displays available count and serving account/quota. | `antiquota/ui/app.js:385` | None identified | [Passed](TEST_REPORT.md#f-741) |
| F-742 | [x] The sidebar account strip shows account quota and paused/wall/attention status. | `antiquota/ui/app.js:385` | None identified | [Passed](TEST_REPORT.md#f-742) |
| F-743 | [x] The account pill opens Usage when accounts exist. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-743) |
| F-744 | [x] The account pill opens Add Google account when no accounts exist. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-744) |

## Agent, permission and stack controls

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-745 | [x] Verifier model saves a concrete verifier model ID. | `antiquota/ui/app.js:2286` | None identified | [Passed](TEST_REPORT.md#f-745) |
| F-746 | [x] Rounds accepts and saves 1–500 rounds. | `antiquota/ui/app.js:2286` | None identified | [Passed](TEST_REPORT.md#f-746) |
| F-747 | [ ] Approval wait accepts and saves 1–240 minutes. | `antiquota/ui/app.js:2286` | None identified | [Pending](TEST_REPORT.md#f-747) |
| F-748 | [x] Retries after a denial accepts and saves 0–10 retries. | `antiquota/ui/app.js:2286` | None identified | [Passed](TEST_REPORT.md#f-748) |
| F-749 | [x] Plan mode explains its read-only planning and execution workflow. | `antiquota/ui/app.js:2286` | None identified | [Passed](TEST_REPORT.md#f-749) |
| F-750 | [ ] Allow rules can be listed, added and individually removed. | `antiquota/ui/app.js:2307` | None identified | [Pending](TEST_REPORT.md#f-750) |
| F-751 | [ ] Deny rules can be listed, added and individually removed. | `antiquota/ui/app.js:2307` | None identified | [Pending](TEST_REPORT.md#f-751) |
| F-752 | [x] Copy from agy CLI replaces differing private permission rules. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-752) |
| F-753 | [x] Rules edits private GEMINI.md with Save and Cancel. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-753) |
| F-754 | [x] New rule creates an always_on template and adds .md when absent. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-754) |
| F-755 | [x] New skill creates SKILL.md with name and description metadata. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-755) |
| F-756 | [x] New hook creates a named PostToolUse command-hook template. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-756) |
| F-757 | [x] Add server creates a named MCP command/argument template. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-757) |
| F-758 | [x] Plugins lists private bundles and imports differing CLI bundles. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-758) |
| F-759 | [x] Stack rows show same, different or source-only state for each supported source. | `antiquota/ui/app.js:2188` | None identified | [Passed](TEST_REPORT.md#f-759) |
| F-760 | [x] Enable or disable preserves a stack item without deleting it. | `antiquota/ui/app.js:2188` | None identified | [Passed](TEST_REPORT.md#f-760) |
| F-761 | [x] Edit opens the private rule or skill text editor. | `antiquota/ui/app.js:2188` | None identified | [Passed](TEST_REPORT.md#f-761) |
| F-762 | [x] Edit opens the private hook or MCP JSON editor. | `antiquota/ui/app.js:2188` | None identified | [Passed](TEST_REPORT.md#f-762) |
| F-763 | [x] Copy or Replace imports the selected source, kind and optional name into the private stack. | `antiquota/ui/app.js:2144` | None identified | [Passed](TEST_REPORT.md#f-763) |
| F-764 | [x] Copy all differing items imports every differing item from its supported source. | `antiquota/ui/app.js:2155` | None identified | [Passed](TEST_REPORT.md#f-764) |
| F-765 | [x] Delete requires confirmation and preserves the source CLI or IDE item. | `antiquota/ui/app.js:2188` | None identified | [Passed](TEST_REPORT.md#f-765) |
| F-766 | [x] Invalid editor JSON is rejected before saving. | `antiquota/ui/app.js:2178` | None identified | [Passed](TEST_REPORT.md#f-766) |
| F-767 | [x] Text editors load the private file, save changes or cancel the editor. | `antiquota/ui/app.js:2171` | None identified | [Passed](TEST_REPORT.md#f-767) |
| F-768 | [x] JSON editors load the private item, save valid changes or cancel the editor. | `antiquota/ui/app.js:2178` | None identified | [Passed](TEST_REPORT.md#f-768) |
| F-769 | [x] New item names allow letters, digits, dot, underscore, colon, at, plus and minus. | `antiquota/ui/app.js:2210` | None identified | [Passed](TEST_REPORT.md#f-769) |
| F-770 | [x] New item names reject unsafe characters and a leading dot. | `antiquota/ui/app.js:2210` | None identified | [Passed](TEST_REPORT.md#f-770) |
| F-771 | [x] An empty stack section displays Nothing here yet. | `antiquota/ui/app.js:250` | None identified | [Passed](TEST_REPORT.md#f-771) |
| F-772 | [ ] Engine diagnostics shows version, URL/PID/ownership, data, stack, agy, hook and counts. | `antiquota/ui/app.js:2318` | None identified | [Pending](TEST_REPORT.md#f-772) |
| F-773 | [x] Engine log opens engine output in the viewer. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-773) |
| F-774 | [ ] Show data folder reveals the private directory in desktop mode. | `antiquota/ui/app.js:2318` | None identified | [Pending](TEST_REPORT.md#f-774) |
| F-775 | [ ] Restart engine confirms before requesting shutdown and reports the expected app restart. | `antiquota/ui/app.js:2318` | None identified | [Pending](TEST_REPORT.md#f-775) |
| F-776 | [x] Copy everything again from agy CLI confirms replacement of private configuration edits. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-776) |

## Account sign-in states

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-777 | [x] Opening Add Google account resets prior code, sign-in state and timers. | `antiquota/ui/app.js:1996` | None identified | [Passed](TEST_REPORT.md#f-777) |
| F-778 | [x] Browser sign-in reserves a tab before requesting the URL to avoid popup blocking. | `antiquota/ui/app.js:2013` | None identified | [Passed](TEST_REPORT.md#f-778) |
| F-779 | [ ] Native sign-in opens the returned consent URL in the system browser. | `antiquota/ui/app.js:2013` | None identified | [Pending](TEST_REPORT.md#f-779) |
| F-780 | [x] Sign-in exposes a manual link when opening the browser fails. | `antiquota/ui/app.js:2013` | None identified | [Passed](TEST_REPORT.md#f-780) |
| F-781 | [x] New sign-in link cancels the previous login before creating another. | `antiquota/ui/app.js:2013` | None identified | [Passed](TEST_REPORT.md#f-781) |
| F-782 | [x] Sign-in countdown highlights its last fifteen seconds and expiry. | `antiquota/ui/app.js:2013` | None identified | [Passed](TEST_REPORT.md#f-782) |
| F-783 | [x] State polling runs every two seconds and accepts completion without pasting a code. | `antiquota/ui/app.js:2013` | None identified | [Passed](TEST_REPORT.md#f-783) |
| F-784 | [x] Returning focus reads a valid code from the clipboard when the field is empty. | `antiquota/ui/app.js:2074` | None identified | [Passed](TEST_REPORT.md#f-784) |
| F-785 | [x] Paste rejects values outside the 4/ code format with at least twenty permitted characters. | `antiquota/ui/app.js:2074` | None identified | [Passed](TEST_REPORT.md#f-785) |
| F-786 | [x] Clipboard access failure explains manual paste. | `antiquota/ui/app.js:2074` | None identified | [Passed](TEST_REPORT.md#f-786) |
| F-787 | [x] Submitting a code disables Add account while its check runs. | `antiquota/ui/app.js:2087` | None identified | [Passed](TEST_REPORT.md#f-787) |
| F-788 | [x] Successful sign-in reports identity, refreshes accounts and closes after 1.2 seconds. | `antiquota/ui/app.js:2051` | None identified | [Passed](TEST_REPORT.md#f-788) |
| F-789 | [x] Verification state disables code controls and shows verification actions. | `antiquota/ui/app.js:2051` | None identified | [Passed](TEST_REPORT.md#f-789) |
| F-790 | [x] Failed sign-in disables expired controls and offers a new sign-in link. | `antiquota/ui/app.js:2051` | None identified | [Passed](TEST_REPORT.md#f-790) |
| F-791 | [x] A code-submission API error restores Add account and displays the error. | `antiquota/ui/app.js:2087` | None identified | [Passed](TEST_REPORT.md#f-791) |
| F-792 | [x] Closing or restarting sign-in cancels its pending login and clears timers. | `antiquota/ui/app.js:2007` | None identified | [Passed](TEST_REPORT.md#f-792) |

## Every slash-command entry point

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-793 | [ ] /help opens the shortcut and command sheet. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-793) |
| F-794 | [ ] /goal opens the dialog or starts a verified goal from its objective argument. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-794) |
| F-795 | [ ] /clear clears the conversation while preserving the transcript. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-795) |
| F-796 | [ ] /compact starts a fresh conversation with handoff summary. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-796) |
| F-797 | [ ] /rewind confirms file rewind before a specified turn. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-797) |
| F-798 | [ ] /export exports the transcript as Markdown. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-798) |
| F-799 | [ ] /init asks agy to create or improve concise project instructions. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-799) |
| F-800 | [ ] /bg starts a background command or explains the required argument. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-800) |
| F-801 | [ ] /loop sets an interval prompt or removes it with off/stop or an empty argument. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-801) |
| F-802 | [ ] /schedule sets a daily-time prompt or removes it with off/stop or an empty argument. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-802) |
| F-803 | [ ] /memory opens GEMINI.md, falls back to AGENTS.md or explains how to create instructions. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-803) |
| F-804 | [ ] /state opens the stored handoff state. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-804) |
| F-805 | [ ] /stop stops the turn and pauses its goal. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-805) |
| F-806 | [ ] /resume resumes a paused goal. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-806) |
| F-807 | [ ] /cleargoal removes the goal. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-807) |
| F-808 | [ ] /new opens the new-session dialog. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-808) |
| F-809 | [ ] /model patches the model ID. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-809) |
| F-810 | [ ] /effort sets a family effort or explains an unavailable effort. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-810) |
| F-811 | [ ] /mode patches the permission mode. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-811) |
| F-812 | [ ] /rename patches the session title. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-812) |
| F-813 | [ ] /quota probes all accounts. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-813) |
| F-814 | [ ] /usage opens Usage settings. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-814) |
| F-815 | [ ] /accounts opens Accounts settings. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-815) |
| F-816 | [ ] /login opens Add Google account. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-816) |
| F-817 | [ ] /settings opens Settings. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-817) |
| F-818 | [ ] /stack is a legacy stack-drawer entry point requiring repair because its renderer is absent. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-818) |
| F-819 | [ ] /permissions opens Permissions settings. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-819) |
| F-820 | [ ] /delete confirms and deletes the selected session. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-820) |
| F-821 | [ ] /skills opens Skills despite being absent from legacy command suggestions. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-821) |
| F-822 | [ ] /hooks opens Hooks despite being absent from legacy command suggestions. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-822) |
| F-823 | [ ] /mcp opens MCP servers despite being absent from legacy command suggestions. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-823) |

## Autocomplete and shortcuts

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-824 | [ ] A slash prefix filters suggestions without case sensitivity. | `antiquota/ui/app.js:1197` | None identified | [Pending](TEST_REPORT.md#f-824) |
| F-825 | [ ] An @ prefix before the caret searches files and inserts the selected path. | `antiquota/ui/app.js:1197` | None identified | [Pending](TEST_REPORT.md#f-825) |
| F-826 | [ ] Stale mention results cannot replace a newer query. | `antiquota/ui/app.js:1197` | None identified | [Pending](TEST_REPORT.md#f-826) |
| F-827 | [ ] ArrowUp and ArrowDown cycle suggestion items. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-827) |
| F-828 | [ ] Tab or Enter applies the selected suggestion before sending. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-828) |
| F-829 | [ ] Escape dismisses suggestions. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-829) |
| F-830 | [ ] Composer blur dismisses suggestions after 100 ms. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-830) |
| F-831 | [ ] Enter sends except during IME composition. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-831) |
| F-832 | [ ] Shift+Enter inserts a newline. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-832) |
| F-833 | [ ] ArrowUp recalls earlier prompts when empty or at the start without Shift. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-833) |
| F-834 | [ ] ArrowDown recalls newer prompts and the saved draft. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-834) |
| F-835 | [ ] Escape stops the selected working session when no suggestion menu is open. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-835) |
| F-836 | [x] Escape closes the context menu. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-836) |
| F-837 | [x] ArrowUp and ArrowDown wrap focus among context-menu items. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-837) |
| F-838 | [ ] Cmd/Ctrl+N opens New session. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-838) |
| F-839 | [ ] Cmd/Ctrl+. stops the turn. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-839) |
| F-840 | [ ] Cmd/Ctrl+, opens Settings. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-840) |
| F-841 | [ ] Cmd/Ctrl+/ opens the shortcut sheet. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-841) |
| F-842 | [ ] Cmd/Ctrl+Shift+M toggles dictation. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-842) |
| F-843 | [ ] Desktop Cmd/Ctrl+= or Cmd/Ctrl++ increases zoom ten percentage points. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-843) |
| F-844 | [ ] Desktop Cmd/Ctrl+- or Cmd/Ctrl+_ decreases zoom ten percentage points. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-844) |
| F-845 | [ ] Desktop Cmd/Ctrl+0 resets zoom. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-845) |
| F-846 | [x] Cmd/Ctrl+K focuses and selects search. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-846) |
| F-847 | [ ] Cmd/Ctrl+J focuses the composer. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-847) |
| F-848 | [ ] Cmd/Ctrl+B toggles the side panel without Shift. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-848) |
| F-849 | [x] Cmd/Ctrl+Backslash toggles split view outside embedded sessions. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-849) |
| F-850 | [x] Cmd/Ctrl+Shift+[ or { selects the previous visible session. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-850) |
| F-851 | [x] Cmd/Ctrl+Shift+] or } selects the next visible session. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-851) |
| F-852 | [ ] Escape closes Settings when no modal dialog is open. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-852) |
| F-853 | [ ] Alt-clicking a session opens it on the right. | `antiquota/ui/app.js:224` | None identified | [Pending](TEST_REPORT.md#f-853) |
| F-854 | [ ] Non-Mac reference labels use Ctrl and Shift rather than Cmd glyphs. | `antiquota/ui/app.js:1527` | None identified | [Pending](TEST_REPORT.md#f-854) |

## Layout, themes and accessibility

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-855 | [ ] The default dark theme uses the dense warm palette and centralized CSS variables. | `antiquota/ui/app.css:5` | None identified | [Pending](TEST_REPORT.md#f-855) |
| F-856 | [ ] The light palette follows prefers-color-scheme and overrides centralized color variables. | `antiquota/ui/app.css:36` | None identified | [Pending](TEST_REPORT.md#f-856) |
| F-857 | [ ] The composer stays docked below an independently scrolling transcript. | `antiquota/ui/app.css:194` | None identified | [Pending](TEST_REPORT.md#f-857) |
| F-858 | [x] Below 860 pixels the sidebar becomes a slide-in overlay with Open and Close controls. | `antiquota/ui/app.css:544` | None identified | [Passed](TEST_REPORT.md#f-858) |
| F-859 | [ ] Below 860 pixels the side panel becomes an overlay. | `antiquota/ui/app.css:544` | None identified | [Pending](TEST_REPORT.md#f-859) |
| F-860 | [ ] Narrow headers omit working-folder and token totals. | `antiquota/ui/app.css:558` | None identified | [Pending](TEST_REPORT.md#f-860) |
| F-861 | [ ] Below 860 pixels Settings uses wrapped navigation above the body. | `antiquota/ui/app.css:627` | None identified | [Pending](TEST_REPORT.md#f-861) |
| F-862 | [ ] Below 680 pixels the shortcut reference uses one column. | `antiquota/ui/app.css:942` | None identified | [Pending](TEST_REPORT.md#f-862) |
| F-863 | [ ] Below 720 pixels usage statistics use two columns. | `antiquota/ui/app.css:993` | None identified | [Pending](TEST_REPORT.md#f-863) |
| F-864 | [ ] Token meter widths below 620 pixels hide session totals and context label. | `antiquota/ui/app.css:659` | None identified | [Pending](TEST_REPORT.md#f-864) |
| F-865 | [ ] Token meter widths below 440 pixels hide available-account count. | `antiquota/ui/app.css:660` | None identified | [Pending](TEST_REPORT.md#f-865) |
| F-866 | [ ] Reduced-motion preference disables animation and transitions. | `antiquota/ui/app.css:564` | None identified | [Pending](TEST_REPORT.md#f-866) |
| F-867 | [ ] The macOS sidebar and narrow header reserve space for traffic lights independently of native zoom. | `antiquota/ui/app.css:798` | None identified | [Pending](TEST_REPORT.md#f-867) |
| F-868 | [x] Split view keeps two content columns below the sidebar-collapse breakpoint. | `antiquota/ui/app.css:1034` | None identified | [Passed](TEST_REPORT.md#f-868) |
| F-869 | [ ] Terminal occupies its pane and gives the drawer a wider terminal layout. | `antiquota/ui/app.css:1019` | None identified | [Pending](TEST_REPORT.md#f-869) |
| F-870 | [ ] Keyboard focus receives a visible accent outline. | `antiquota/ui/app.css:82` | None identified | [Pending](TEST_REPORT.md#f-870) |
| F-871 | [ ] Hidden controls do not occupy layout space. | `antiquota/ui/app.css:83` | None identified | [Pending](TEST_REPORT.md#f-871) |

## Native scale behavior

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-872 | [ ] Desktop scale uses native webview zoom rather than CSS zoom. | `antiquota/ui/app.js:81` | tests/ui.test.cjs:30 | [Pending](TEST_REPORT.md#f-872) |
| F-873 | [ ] Desktop scale persists per device and restores when the root webview opens. | `antiquota/ui/app.js:81` | tests/ui.test.cjs:30 | [Pending](TEST_REPORT.md#f-873) |

## Embedded scale behavior

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-874 | [ ] Embedded scale controls delegate to the root without applying another scale. | `antiquota/ui/app.js:81` | tests/ui.test.cjs:47 | [Pending](TEST_REPORT.md#f-874) |

## Browser scale behavior

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-875 | [ ] Browser scale stays under the browser's own zoom controls. | `antiquota/ui/app.js:81` | tests/ui.test.cjs:57 | [Pending](TEST_REPORT.md#f-875) |

## Scale validation

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-876 | [ ] Invalid scale resets to one and extreme scale clamps to 70–160 percent. | `antiquota/ui/app.js:81` | tests/ui.test.cjs:65 | [Pending](TEST_REPORT.md#f-876) |

## Every native application menu action

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-877 | [ ] antiquota > About antiquota shows native application metadata. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-877) |
| F-878 | [ ] antiquota > Settings… opens General; Cmd/Ctrl+Comma. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-878) |
| F-879 | [ ] antiquota > Accounts and Usage opens Usage. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-879) |
| F-880 | [ ] antiquota > Services exposes platform services. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-880) |
| F-881 | [ ] antiquota > Hide hides the application. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-881) |
| F-882 | [ ] antiquota > Hide Others hides other applications. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-882) |
| F-883 | [ ] antiquota > Show All restores hidden applications. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-883) |
| F-884 | [ ] antiquota > Quit antiquota uses work-aware quit; Cmd/Ctrl+Q. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-884) |
| F-885 | [ ] File > New Session opens session creation; Cmd/Ctrl+N. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-885) |
| F-886 | [ ] File > Export Transcript… exports the transcript; Cmd/Ctrl+Shift+E. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-886) |
| F-887 | [ ] File > Close Window hides the main window into the tray. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-887) |
| F-888 | [ ] Edit > Undo uses native undo. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-888) |
| F-889 | [ ] Edit > Redo uses native redo. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-889) |
| F-890 | [ ] Edit > Cut cuts focused content to the clipboard. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-890) |
| F-891 | [ ] Edit > Copy copies focused content to the clipboard. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-891) |
| F-892 | [ ] Edit > Paste pastes clipboard content. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-892) |
| F-893 | [ ] Edit > Select All selects focused editable content. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-893) |
| F-894 | [ ] Edit > Find Session or Message focuses and selects search; Cmd/Ctrl+K. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-894) |
| F-895 | [ ] Edit > Dictate toggles dictation; Cmd/Ctrl+Shift+M. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-895) |
| F-896 | [ ] View > Zoom In increases native zoom; Cmd/Ctrl+Equal. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-896) |
| F-897 | [ ] View > Zoom Out decreases native zoom; Cmd/Ctrl+Minus. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-897) |
| F-898 | [ ] View > Actual Size resets native zoom; Cmd/Ctrl+0. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-898) |
| F-899 | [ ] View > Side Panel toggles the drawer; Cmd/Ctrl+B. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-899) |
| F-900 | [ ] View > Side by Side toggles split; Cmd/Ctrl+Backslash. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-900) |
| F-901 | [ ] View > Expand All Steps toggles transcript expansion; Cmd/Ctrl+Shift+O. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-901) |
| F-902 | [ ] View > Reload reloads the page; Cmd/Ctrl+R. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-902) |
| F-903 | [ ] View > Fullscreen uses the platform fullscreen action. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-903) |
| F-904 | [ ] Window > Minimize uses the platform minimize action. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-904) |
| F-905 | [ ] Window > Maximize uses the platform maximize action. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-905) |
| F-906 | [ ] Window > Previous Session opens the previous session; Cmd/Ctrl+Shift+BracketLeft. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-906) |
| F-907 | [ ] Window > Next Session opens the next session; Cmd/Ctrl+Shift+BracketRight. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-907) |
| F-908 | [ ] Help > Keyboard Shortcuts opens shortcuts; Cmd/Ctrl+Slash. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-908) |
| F-909 | [ ] Help > Engine Log opens the engine log viewer. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-909) |

## Desktop lifecycle and window state

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-910 | [ ] Showing the main window unminimizes, reveals and focuses it. | `app/src-tauri/src/lib.rs:24` | None identified | [Pending](TEST_REPORT.md#f-910) |
| F-911 | [ ] A second instance focuses the existing main window rather than opening another engine. | `app/src-tauri/src/lib.rs:137` | None identified | [Pending](TEST_REPORT.md#f-911) |
| F-912 | [ ] The shell persists window size, position and maximized state without visibility. | `app/src-tauri/src/lib.rs:140` | None identified | [Pending](TEST_REPORT.md#f-912) |
| F-913 | [ ] The default or unavailable-monitor window position is centered. | `app/src-tauri/src/lib.rs:163` | None identified | [Pending](TEST_REPORT.md#f-913) |
| F-914 | [ ] Default window dimensions are 1280×820 with a 720×480 minimum. | `app/src-tauri/src/lib.rs:161` | None identified | [Pending](TEST_REPORT.md#f-914) |
| F-915 | [ ] --hidden starts without showing the main window. | `app/src-tauri/src/lib.rs:167` | None identified | [Pending](TEST_REPORT.md#f-915) |
| F-916 | [ ] Autostart uses a macOS LaunchAgent with --hidden. | `app/src-tauri/src/lib.rs:148` | None identified | [Pending](TEST_REPORT.md#f-916) |
| F-917 | [ ] The macOS title bar overlays content and traffic lights use position 18,20. | `app/src-tauri/src/lib.rs:171` | None identified | [Pending](TEST_REPORT.md#f-917) |
| F-918 | [ ] HTML drag-and-drop reaches the UI for file attachments. | `app/src-tauri/src/lib.rs:166` | None identified | [Pending](TEST_REPORT.md#f-918) |
| F-919 | [ ] Closing the main window hides it into the tray while the engine keeps running. | `app/src-tauri/src/lib.rs:182` | None identified | [Pending](TEST_REPORT.md#f-919) |
| F-920 | [ ] Reopening from the macOS dock reveals the main window. | `app/src-tauri/src/lib.rs:197` | None identified | [Pending](TEST_REPORT.md#f-920) |
| F-921 | [ ] Boot discovers or starts the engine and navigates the main window to it. | `app/src-tauri/src/lib.rs:37` | None identified | [Pending](TEST_REPORT.md#f-921) |
| F-922 | [ ] Concurrent boot requests are suppressed. | `app/src-tauri/src/lib.rs:37` | None identified | [Pending](TEST_REPORT.md#f-922) |
| F-923 | [ ] Splash retry starts another boot in a worker thread. | `app/src-tauri/src/lib.rs:62` | None identified | [Pending](TEST_REPORT.md#f-923) |
| F-924 | [ ] The app-info command returns version and operating system. | `app/src-tauri/src/lib.rs:57` | None identified | [Pending](TEST_REPORT.md#f-924) |
| F-925 | [ ] Native save writes contents at the user-selected path or returns cancellation. | `app/src-tauri/src/lib.rs:68` | None identified | [Pending](TEST_REPORT.md#f-925) |
| F-926 | [ ] Quitting does not shut down an engine started outside the app. | `app/src-tauri/src/lib.rs:83` | None identified | [Pending](TEST_REPORT.md#f-926) |
| F-927 | [ ] Quitting an app-owned idle engine requests shutdown. | `app/src-tauri/src/lib.rs:83` | None identified | [Pending](TEST_REPORT.md#f-927) |
| F-928 | [ ] Quitting with active sessions or jobs presents the work-aware choice. | `app/src-tauri/src/lib.rs:83` | None identified | [Pending](TEST_REPORT.md#f-928) |
| F-929 | [ ] Keep running in background quits the shell and retains the engine. | `app/src-tauri/src/lib.rs:83` | None identified | [Pending](TEST_REPORT.md#f-929) |
| F-930 | [ ] Stop and quit stops turns while background jobs continue. | `app/src-tauri/src/lib.rs:83` | None identified | [Pending](TEST_REPORT.md#f-930) |

## Engine discovery and process startup

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-931 | [ ] Discovery honors ANTIQUOTA_DIR or uses ~/.antiquota. | `app/src-tauri/src/engine.rs:28` | None identified | [Pending](TEST_REPORT.md#f-931) |
| F-932 | [ ] Discovery reads state/engine.json and GUI token to attach to the shared engine. | `app/src-tauri/src/engine.rs:64` | None identified | [Pending](TEST_REPORT.md#f-932) |
| F-933 | [ ] Discovery accepts only a health response identifying antiquota. | `app/src-tauri/src/engine.rs:51` | None identified | [Pending](TEST_REPORT.md#f-933) |
| F-934 | [ ] Ownership in engine.json controls shutdown behavior across later app starts. | `app/src-tauri/src/engine.rs:64` | None identified | [Pending](TEST_REPORT.md#f-934) |
| F-935 | [ ] Native engine requests have a five-second global timeout. | `app/src-tauri/src/engine.rs:39` | None identified | [Pending](TEST_REPORT.md#f-935) |
| F-936 | [ ] ANTIQUOTA_ENGINE takes precedence over bundled and source engines. | `app/src-tauri/src/engine.rs:98` | None identified | [Pending](TEST_REPORT.md#f-936) |
| F-937 | [ ] A release discovers its platform engine in bundled resources. | `app/src-tauri/src/engine.rs:98` | None identified | [Pending](TEST_REPORT.md#f-937) |
| F-938 | [ ] Legacy source discovery honors ANTIQUOTA_DEV_ROOT or repository location. | `app/src-tauri/src/engine.rs:98` | None identified | [Pending](TEST_REPORT.md#f-938) |
| F-939 | [ ] Legacy discovery searches for Python. Authorized replacement: discover the single Rust antiquota sidecar or an explicit Rust-engine executable while retaining shared engine.json discovery and actionable missing-engine errors; no Python interpreter is required. | `app/src-tauri/src/engine.rs:128` | None identified | [Pending](TEST_REPORT.md#f-939) |
| F-940 | [ ] Spawned engine output appends to private state/engine.log. | `app/src-tauri/src/engine.rs:150` | None identified | [Pending](TEST_REPORT.md#f-940) |
| F-941 | [ ] App engine launch supplies serve --port 8765 and app ownership/path markers. | `app/src-tauri/src/engine.rs:150` | None identified | [Pending](TEST_REPORT.md#f-941) |
| F-942 | [ ] POSIX engines use an independent process group and survive app exit. | `app/src-tauri/src/engine.rs:174` | None identified | [Pending](TEST_REPORT.md#f-942) |
| F-943 | [ ] Windows engines use a new process group without a console window. | `app/src-tauri/src/engine.rs:180` | None identified | [Pending](TEST_REPORT.md#f-943) |
| F-944 | [ ] Startup polls engine discovery every 250 ms up to thirty seconds. | `app/src-tauri/src/engine.rs:188` | None identified | [Pending](TEST_REPORT.md#f-944) |
| F-945 | [ ] Startup failure reports timeout with up to twelve engine-log tail lines. | `app/src-tauri/src/engine.rs:188` | None identified | [Pending](TEST_REPORT.md#f-945) |

## Every tray item and tray state

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-946 | [ ] The tray uses a template icon and opens its menu on left click. | `app/src-tauri/src/tray.rs:13` | None identified | [Pending](TEST_REPORT.md#f-946) |
| F-947 | [ ] The disabled tray heading shows starting, idle, active sessions and jobs. | `app/src-tauri/src/tray.rs:107` | None identified | [Pending](TEST_REPORT.md#f-947) |
| F-948 | [ ] Up to six working sessions appear with truncated title and status. | `app/src-tauri/src/tray.rs:107` | None identified | [Pending](TEST_REPORT.md#f-948) |
| F-949 | [ ] Selecting a tray session reveals the main window and opens that session. | `app/src-tauri/src/tray.rs:26` | None identified | [Pending](TEST_REPORT.md#f-949) |
| F-950 | [ ] Up to ten accounts appear as disabled quota/status rows. | `app/src-tauri/src/tray.rs:107` | None identified | [Pending](TEST_REPORT.md#f-950) |
| F-951 | [ ] Account rows distinguish Gemini quota, wall, cooldown, signed-out and unprobed state. | `app/src-tauri/src/tray.rs:80` | None identified | [Pending](TEST_REPORT.md#f-951) |
| F-952 | [ ] Rewrite correction: tray verification-needed state accepts the actual account API verification URL string, as well as compatible legacy object/boolean representations. The URL-string mismatch is confirmed by source, not claimed as observed during the Gemini run. | `app/src-tauri/src/tray.rs:80` | None identified | [Pending](TEST_REPORT.md#f-952) |
| F-953 | [ ] Tray Open antiquota reveals the main window. | `app/src-tauri/src/tray.rs:147` | None identified | [Pending](TEST_REPORT.md#f-953) |
| F-954 | [ ] Tray New session… reveals the window and opens session creation. | `app/src-tauri/src/tray.rs:148` | None identified | [Pending](TEST_REPORT.md#f-954) |
| F-955 | [ ] Tray Stop all running is enabled only while sessions work and stops every active session. | `app/src-tauri/src/tray.rs:149` | None identified | [Pending](TEST_REPORT.md#f-955) |
| F-956 | [ ] Tray Quit antiquota uses work-aware quit and Cmd/Ctrl+Q. | `app/src-tauri/src/tray.rs:151` | None identified | [Pending](TEST_REPORT.md#f-956) |
| F-957 | [ ] Tray state refreshes every five seconds. | `app/src-tauri/src/tray.rs:156` | None identified | [Pending](TEST_REPORT.md#f-957) |
| F-958 | [ ] Three consecutive state-request failures trigger engine discovery or respawn. | `app/src-tauri/src/tray.rs:156` | None identified | [Pending](TEST_REPORT.md#f-958) |
| F-959 | [ ] On macOS, tray title, tooltip and dock/window badge indicate active-session count. | `app/src-tauri/src/tray.rs:156` | None identified | [Pending](TEST_REPORT.md#f-959) |

## Splash and native permissions

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-960 | [ ] The splash displays startup progress from engine-status events. | `app/splash/index.html:38` | None identified | [Pending](TEST_REPORT.md#f-960) |
| F-961 | [ ] The splash displays error text with Try again and Quit only in error state. | `app/splash/index.html:51` | None identified | [Pending](TEST_REPORT.md#f-961) |
| F-962 | [ ] Try again clears the error state and invokes retry_engine. | `app/splash/index.html:54` | None identified | [Pending](TEST_REPORT.md#f-962) |
| F-963 | [ ] Splash Quit invokes work-aware quit_app. | `app/splash/index.html:59` | None identified | [Pending](TEST_REPORT.md#f-963) |
| F-964 | [ ] The splash respects reduced-motion preference. | `app/splash/index.html:34` | None identified | [Pending](TEST_REPORT.md#f-964) |

## Native permissions scope

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-965 | [ ] The named main window grants engine-page APIs to localhost and 127.0.0.1 origins. | `app/src-tauri/capabilities/engine-ui.json:6` | None identified | [Pending](TEST_REPORT.md#f-965) |
| F-966 | [ ] Engine-page permissions expose zoom, dragging, attention, badges, clipboard, notification, autostart and save actions. | `app/src-tauri/capabilities/engine-ui.json:11` | None identified | [Pending](TEST_REPORT.md#f-966) |
| F-967 | [ ] Engine-page openPath capability is limited to the user's home directory. | `app/src-tauri/capabilities/engine-ui.json:21` | None identified | [Pending](TEST_REPORT.md#f-967) |

## Legacy packaging and release outputs

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-968 | [ ] Legacy engine resources use an engine/antiquotad tree. Authorized replacement: bundle the single Rust antiquota sidecar through the platform Tauri configuration and discover it while preserving shared engine.json sessions; retain no PyInstaller dependency. | `app/src-tauri/tauri.bundle.json:3` | None identified | [Pending](TEST_REPORT.md#f-968) |

## Legacy bundled command entry point

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-969 | [ ] The bundled engine forwards serve and current CLI commands to the engine entry point. | `app/engine/antiquotad.py:4` | None identified | [Pending](TEST_REPORT.md#f-969) |

## Platform release packages

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-970 | [ ] macOS release builds an app and compressed DMG with an Applications shortcut. | `app/scripts/build-app.sh:7` | None identified | [Pending](TEST_REPORT.md#f-970) |
| F-971 | [ ] Linux release builds deb, rpm and AppImage outputs. | `app/scripts/build-app.sh:21` | None identified | [Pending](TEST_REPORT.md#f-971) |
| F-972 | [ ] Other release platforms use the configured Tauri bundle targets. | `app/scripts/build-app.sh:24` | None identified | [Pending](TEST_REPORT.md#f-972) |

## Legacy CI matrix

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-973 | [ ] CI independently builds desktop artifacts on macOS, Windows and Linux. | `.github/workflows/app.yml:12` | None identified | [Pending](TEST_REPORT.md#f-973) |
| F-974 | [ ] Authorized CI replacement: run cargo tests and clippy, pnpm check, Vitest, Playwright, stale-type checks and the Rust-sidecar app build on macOS, Windows and Linux; legacy Python test coverage remains reference evidence only. | `.github/workflows/app.yml:33` | None identified | [Pending](TEST_REPORT.md#f-974) |
| F-975 | [ ] Authorized Windows replacement: bundle the Rust sidecar with portable-pty ConPTY support and verify paths, terminal operation and installer packaging; no pywinpty runtime dependency is required. | `.github/workflows/app.yml:42` | None identified | [Pending](TEST_REPORT.md#f-975) |
| F-976 | [ ] Authorized POSIX replacement: bundle the self-contained Rust antiquota sidecar, verify its shared-engine discovery and build native installers without Python or PyInstaller. | `.github/workflows/app.yml:45` | None identified | [Pending](TEST_REPORT.md#f-976) |
| F-977 | [ ] CI uploads platform app bundles and installers. | `.github/workflows/app.yml:51` | None identified | [Pending](TEST_REPORT.md#f-977) |

## Dialog defaults and remaining interaction edges

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-978 | [ ] New-session defaults use the current model/mode and project folder, or configured defaults without a session. | `antiquota/ui/app.js:1948` | None identified | [Pending](TEST_REPORT.md#f-978) |
| F-979 | [ ] New sessions from a worktree default to the parent repository folder. | `antiquota/ui/app.js:1948` | None identified | [Pending](TEST_REPORT.md#f-979) |
| F-980 | [ ] The worktree checkbox follows the configured default unless an explicit project-menu choice overrides it. | `antiquota/ui/app.js:1948` | None identified | [Pending](TEST_REPORT.md#f-980) |
| F-981 | [ ] Working-folder browse shows up to four recent unique session folders before a path is typed. | `antiquota/ui/app.js:1934` | None identified | [Pending](TEST_REPORT.md#f-981) |
| F-982 | [ ] Folder browser entries navigate into a child folder or up to its parent. | `antiquota/ui/app.js:1934` | None identified | [Pending](TEST_REPORT.md#f-982) |
| F-983 | [ ] Selecting a browser folder updates the working-folder input. | `antiquota/ui/app.js:1934` | None identified | [Pending](TEST_REPORT.md#f-983) |
| F-984 | [ ] Typing a working folder debounces filesystem browsing by 200 ms. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-984) |
| F-985 | [ ] Create session posts folder, concrete effort/model, mode and worktree choice. | `antiquota/ui/app.js:1959` | None identified | [Pending](TEST_REPORT.md#f-985) |
| F-986 | [ ] Successful creation inserts the session into navigation and opens it. | `antiquota/ui/app.js:1959` | None identified | [Pending](TEST_REPORT.md#f-986) |
| F-987 | [ ] A creation error shows the API error without creating a fake local session. | `antiquota/ui/app.js:1959` | None identified | [Pending](TEST_REPORT.md#f-987) |
| F-988 | [ ] Canceling New session closes its dialog without sending a create request. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-988) |
| F-989 | [ ] Canceling Goal closes its dialog without sending a goal request. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-989) |
| F-990 | [ ] Goal verifier defaults to enabled unless the existing goal says otherwise. | `antiquota/ui/app.js:1970` | None identified | [Pending](TEST_REPORT.md#f-990) |
| F-991 | [ ] Goal rounds default to twenty unless the existing goal has a limit. | `antiquota/ui/app.js:1970` | None identified | [Pending](TEST_REPORT.md#f-991) |
| F-992 | [ ] Goal objective and working folder use required input validation before accepting their forms. | `antiquota/ui/index.html:148` | None identified | [Pending](TEST_REPORT.md#f-992) |
| F-993 | [ ] The sign-in code submits on Enter and automatically after paste. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-993) |
| F-994 | [ ] A raw text paste without files remains ordinary composer input. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-994) |
| F-995 | [ ] Live turn completion triggers turn notification except for quota endings. | `antiquota/ui/app.js:871` | None identified | [Pending](TEST_REPORT.md#f-995) |
| F-996 | [ ] Live job completion triggers the job-finished notification. | `antiquota/ui/app.js:871` | None identified | [Pending](TEST_REPORT.md#f-996) |
| F-997 | [ ] A live met-goal notice triggers the goal-met notification. | `antiquota/ui/app.js:871` | None identified | [Pending](TEST_REPORT.md#f-997) |
| F-998 | [ ] A live exhausted-quota notice triggers the quota notification. | `antiquota/ui/app.js:871` | None identified | [Pending](TEST_REPORT.md#f-998) |
| F-999 | [ ] Desktop header and brand regions permit native window dragging. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-999) |
| F-1000 | [ ] Desktop external-link handling preserves in-app file links and downloadable files. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-1000) |
| F-1001 | [ ] Session relative-time labels refresh every minute. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-1001) |
| F-1002 | [ ] An unknown slash command shows its unknown-command message. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-1002) |
| F-1003 | [ ] Commands requiring a session report Open a session first when none is open. | `antiquota/ui/app.js:1430` | None identified | [Pending](TEST_REPORT.md#f-1003) |

## Requested rewrite scale acceptance

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-1004 | [ ] Both split panes remain fully visible and equally scaled at 80, 100, 125 and 150 percent native zoom. | `antiquota/ui/app.js:81` | None identified | [Pending](TEST_REPORT.md#f-1004) |
| F-1005 | [ ] Context menus and usage tooltips track their intended position at every required zoom. | `antiquota/ui/app.js:81` | None identified | [Pending](TEST_REPORT.md#f-1005) |
| F-1006 | [ ] Terminal fitting remains correct at every required zoom. | `antiquota/ui/app.js:81` | None identified | [Pending](TEST_REPORT.md#f-1006) |
| F-1007 | [ ] Session sidebar content stays clear of macOS traffic lights at every required zoom. | `antiquota/ui/app.js:81` | None identified | [Pending](TEST_REPORT.md#f-1007) |

## Requested rewrite desktop persistence acceptance

| ID | Acceptance behavior | Legacy source | Existing legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-1008 | [ ] Window size, position and maximized state survive a real quit and relaunch. | `app/src-tauri/src/lib.rs:137` | None identified | [Pending](TEST_REPORT.md#f-1008) |
| F-1009 | [ ] A saved position on a disconnected monitor falls back to a visible centered window. | `app/src-tauri/src/lib.rs:163` | None identified | [Pending](TEST_REPORT.md#f-1009) |

## Inventory boundaries and source risks

- The legacy light theme follows the operating-system preference; there is no manual theme selector. Requested light/dark tests must exercise both palettes.
- The native scale tests above prove control behavior only. Screenshot and desktop restart acceptance remains Pending.
- `/skills`, `/hooks` and `/mcp` work as typed entry points but are missing from legacy command suggestions. `/stack` selects a drawer state with no renderer. Repair these inconsistencies rather than silently dropping commands.
- Legacy Python discovery and PyInstaller packaging are recorded for replacement with the explicitly requested Rust sidecar. They are not requirements to retain Python.
- Source review exposes unverified risks: empty model families dereference a missing family; switch callbacks retain their original boolean; the settings heading helper uses a comma expression; tray verification formatting accepts objects or booleans while the API can supply a URL string. Reproduce these before calling them confirmed runtime defects.
- Menu separators and decorative icons have no separate command ID. Disabled tray headings and account rows are observable status features, recorded above.
- Implementation architecture requirements such as generated API types and a single typed client belong in the architecture acceptance inventory; they do not replace any legacy UI behavior.
