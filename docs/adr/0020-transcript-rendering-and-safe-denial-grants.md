# ADR 0020: Transcript rendering and safe denial grants

Date: 2026-10-08

Status: Accepted

The transcript uses the legacy Markdown grammar after escaping model content. Only generated HTTP links, file-viewer links and code-copy buttons become interactive. Code and completed-message copy retain their original contents. Tool labels, previews, grouped counts, prompt origins and notice states live in pure helpers. Adjacent tool steps form stable groups; reasoning, replies, notices and turn boundaries retain their arrival order. Detailed and compact presentation use the existing device preferences, while explicit expansion remains local to each pane.

The shared file viewer admits one latest read across Markdown links, the Files drawer and composer memory/state commands. Deferred reads retain their initiating session and cannot replace newer viewer content. Transcript actions preserve newer drafts and selection, grant permission before sending a retry, and propagate failures without sending the retry.

Denial notices receive rule suggestions from core policy, including historical HTTP views. Existing durable records retain their original bytes. Arbitrary runners, executable expansions and risky flags do not produce permanent suggestions. Permanent rule validation recognizes absolute, quoted, escaped, versioned and Windows program identities. Loaded rule matching and deny precedence remain compatible; risky flags still require a decision.

The existing permissions PUT accepts an additive `add_allow` list. It cannot be combined with list replacements. The adapter validates the entire batch and appends under the same lock used by replacements; it preserves deny rules and unknown settings. Existing approval grants reuse this operation. Invalid batches and malformed settings fail without replacing their source bytes. The captured allow/deny replacement request remains unchanged.

Rewind retains the B-022 correction: later user edits prevent restoration before any file is written. Historical rewind notices still identify outside edits. Browser proof uses the isolated fake agent and headless Chromium; native WebKit, menus and live authorization remain in Slice 10.
