# ADR 0023: Storage location and persisted defaults

Status: Accepted for Slice 11.

antiquota keeps its state in `ANTIQUOTA_DIR` when it is set, otherwise in `.antiquota` under the user's home. The home is `ANTIQUOTA_HOME` when set; on POSIX it otherwise comes from the account database, not `HOME`, because inside an account's agy process `HOME` is that account's folder. Windows uses `USERPROFILE`.

The first time the default folder is resolved in a process, a real `~/.agyloop` folder (the project's old name) moves to `~/.antiquota` when that does not exist yet. Links inside it, the own-stack config files and the conversation store link that name the old folder are pointed at the new one, and a link is left at the old path so paths in old transcripts still open. A linked or missing `~/.agyloop`, or an existing `~/.antiquota`, means nothing moves, so retries are no-ops.

`settings.json` is read key by key, as the legacy reader did. A number or boolean written by hand becomes its text, an unusable value falls back to that setting's default, unknown keys are kept, and only unreadable JSON resets everything. Previously one non-string value reset every setting. Saves still validate every known value and refuse a bad stored number by field name until it is replaced. Numbers are trimmed and stored by their shortest form.
