#!/usr/bin/env python3
"""Stand-in for agy: names the account by its HOME and spends no quota.

STUB_429=<name>    that account fails with a quota error
STUB_SPENT=<name>  that account's quota panel shows the Gemini 5h window spent
STUB_SAY=<text>    extra stdout line, as if the model said it
"""
import json
import os
import sys

home = os.environ["HOME"]
name = os.path.basename(home)
args = sys.argv[1:]

if not args:  # interactive sign-in
    d = os.path.join(home, ".gemini/antigravity-cli")
    os.makedirs(d, exist_ok=True)
    open(os.path.join(d, "jetski_state.pbtxt"), "w").write(os.environ.get("STUB_ACCOUNT", "acct"))
    os.makedirs(os.path.join(home, ".gemini/config"), exist_ok=True)
    open(os.path.join(home, ".gemini/config/config.json"), "w").write("{}")
    sys.exit(0)

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

print(f"ran on {name}: {' '.join(args)}")
if os.environ.get("STUB_SAY"):
    print(os.environ["STUB_SAY"])
if os.environ.get("STUB_429") == name:
    print("Error: RESOURCE_EXHAUSTED: Individual quota reached", file=sys.stderr)
    sys.exit(1)
