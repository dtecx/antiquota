#!/usr/bin/env python3
"""Stand-in for agy: names the account by its HOME and spends no quota.

STUB_429=<name>    that account fails with a quota error
STUB_SPENT=<name>  that account's quota panel shows the Gemini 5h window spent
STUB_SAY=<text>    extra stdout line, as if the model said it

With -p <prompt> --output-format stream-json it plays one agent turn in the
current folder: edits notes.txt, streams a reply, and logs its argv to
.stub_calls.jsonl. The reply ends with GOAL_COMPLETE from turn STUB_DONE_AFTER
(default 1). STUB_FLAG_AT=<n> creates the file "flag" on turn n. STUB_SLEEP
pauses mid-turn. Asked for a VERDICT it is the verifier: STUB_VERDICT=done|no.
"""
import json
import os
import sys

home = os.environ["HOME"]
name = os.path.basename(home)
args = sys.argv[1:]

auth = os.path.join(home, ".gemini/antigravity-cli/jetski_state.pbtxt")
if args[:2] == ["-p", "/quota"] and not os.path.exists(auth):  # unsigned: agy's headless sign-in
    print("Authentication required. Please visit the URL to log in:", file=sys.stderr)
    print("  https://accounts.google.com/o/oauth2/auth?client_id=stub&state=stub", file=sys.stderr, flush=True)
    print("Or, paste the authorization code here and press Enter:", file=sys.stderr, flush=True)
    os.makedirs(os.path.dirname(auth), exist_ok=True)
    open(auth, "w").write("")  # agy writes an empty state before sign-in
    os.makedirs(os.path.join(home, ".gemini/config"), exist_ok=True)
    open(os.path.join(home, ".gemini/config/config.json"), "w").write("{}")
    log = args[args.index("--log-file") + 1] if "--log-file" in args else os.devnull
    if sys.stdin.readline().strip() != "good-code":
        print(json.dumps({"status": "ERROR", "error": "authentication failed or timed out"}))
        print("error: authentication failed or timed out", file=sys.stderr)
        sys.exit(1)
    open(auth, "w").write(os.environ.get("STUB_ACCOUNT", "acct"))
    with open(log, "a") as f:
        f.write(f"I1006 server_oauth.go:203] OAuth: authenticated successfully as {os.environ.get('STUB_EMAIL', 'acct@example.com')}\n")

if args[:2] == ["-p", "/quota"]:
    d = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "quota_sample.json")))
    spent = os.environ.get("STUB_SPENT") == name
    for g in d["command"]["data"]["groups"]:
        for b in g["buckets"]:
            five = b["window"] == "5h"
            b["remaining_fraction"] = 0.01 if spent and five and g["name"].startswith("Gemini") else 0.9
            b["reset_time"] = "2099-01-01T05:00:00Z" if five else "2099-01-07T00:00:00Z"
    print(json.dumps(d))
    sys.exit(0)

if args[:2] in (["-p", "/skills"], ["-p", "/hooks"]):  # read through HOME, like agy
    cfg = os.path.join(home, ".gemini/config")
    if args[1] == "/skills":
        items = sorted(os.listdir(os.path.join(cfg, "skills")))
    else:
        items = sorted(json.load(open(os.path.join(cfg, "hooks.json"))))
    key = args[1][1:]
    print(json.dumps({"command": {"name": key, "data": {key: [{"name": n} for n in items]}}}))
    sys.exit(0)


def emit(**d):
    print(json.dumps(d), flush=True)


if args[:1] == ["-p"] and "--output-format" in args and not args[1].startswith("/"):
    conv = args[args.index("--conversation") + 1] if "--conversation" in args else f"conv-{name}-{os.getpid()}"
    with open(".stub_calls.jsonl", "a") as f:
        f.write(json.dumps({"slot": name, "args": args}) + "\n")
    if "VERDICT:" in args[1]:
        ok = os.environ.get("STUB_VERDICT", "done") == "done"
        reply = "All good.\n**VERDICT: DONE**" if ok else "Not yet.\n- tests are missing\nVERDICT: NOT_DONE"
        emit(conversation_id=conv, status="SUCCESS", response=reply)
        sys.exit(0)
    if os.environ.get("STUB_429") == name:
        emit(event="result", result={"conversation_id": conv, "status": "ERROR",
                                     "error": "RESOURCE_EXHAUSTED: Individual quota reached"})
        print("Error: RESOURCE_EXHAUSTED", file=sys.stderr)
        sys.exit(1)
    with open(".stub_turns", "a") as f:
        f.write("x")
    n = len(open(".stub_turns").read())
    emit(event="init", conversation_id=conv, init={"model": "stub", "cwd": os.getcwd()})
    target = os.path.join(os.getcwd(), "notes.txt")
    step = {"conversation_id": conv, "step_index": 3, "step_type": "tool", "tool_name": "write_to_file",
            "tool_info": {"name": "write_to_file", "parameters": {"TargetFile": target}}}
    emit(event="step_update", step_update={**step, "state": "ACTIVE"})
    import time
    time.sleep(0.2)  # agy checks permissions before the tool runs
    with open(target, "a") as f:
        f.write(f"turn {n}\n")
    if os.environ.get("STUB_FLAG_AT") == str(n):
        open("flag", "w").close()
    if os.environ.get("STUB_SLEEP"):
        time.sleep(float(os.environ["STUB_SLEEP"]))
    emit(event="step_update", step_update={**step, "state": "DONE", "duration_seconds": 0.01})
    reply = f"turn {n} on {name}"
    if n >= int(os.environ.get("STUB_DONE_AFTER", "1")):
        reply += "\nGOAL_COMPLETE"
    for part in (reply[:5], reply[5:]):
        emit(event="step_update", step_update={"conversation_id": conv, "step_index": 4, "state": "ACTIVE",
                                               "step_type": "agent_response", "text_delta": part})
    emit(event="step_update", step_update={"conversation_id": conv, "step_index": 4, "state": "DONE",
                                           "step_type": "agent_response", "text_delta": ""})
    emit(event="result", result={"conversation_id": conv, "status": "SUCCESS", "response": reply,
                                 "usage": {"input_tokens": 100, "output_tokens": 10, "total_tokens": 110}})
    sys.exit(0)

print(f"ran on {name}: {' '.join(args)}")
if os.environ.get("STUB_SAY"):
    print(os.environ["STUB_SAY"])
if os.environ.get("STUB_429") == name:
    print("Error: RESOURCE_EXHAUSTED: Individual quota reached", file=sys.stderr)
    sys.exit(1)
