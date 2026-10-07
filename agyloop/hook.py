"""agy hook commands that connect a running agy turn to the agy-loop engine.

agy runs these for the turns agy-loop starts (it passes AGYLOOP_SESSION and AGYLOOP_STATE):

  pre-tool        PreToolUse: commands and edits that need the user's yes are asked in the GUI,
                  and agy waits for the answer instead of the headless turn dying on a denial.
  pre-invocation  PreInvocation: messages the user sent while agy works are handed to agy before
                  its next model call, so it reads them mid-turn.

Standard library only, and no import of the rest of agy-loop: it starts fast for every tool call.
Any failure answers in a way that leaves agy's own behaviour unchanged.
"""
import json
import os
import sys
import time
import urllib.request

WAIT = 25          # seconds per long poll while the user decides


def _engine():
    state = os.environ.get("AGYLOOP_STATE", "")
    with open(os.path.join(state, "engine.json")) as f:
        url = json.load(f)["url"]
    with open(os.path.join(state, "gui-token")) as f:
        return url, f.read().strip()


def _call(method, path, body=None, timeout=WAIT + 10):
    url, token = _engine()
    req = urllib.request.Request(url + "api/" + path, method=method,
                                 data=json.dumps(body).encode() if body is not None else None,
                                 headers={"X-Agyloop-Token": token, "Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read() or b"{}")


def pre_tool(payload, sid):
    call = payload.get("toolCall") or {}
    r = _call("POST", "approvals", {"session": sid, "tool": call.get("name", ""), "args": call.get("args") or {},
                                    "conversation": payload.get("conversationId"), "step": payload.get("stepIdx")})
    while "pending" in r:
        r = _call("GET", f"approvals/{r['pending']}?wait={WAIT}")
    out = {"decision": r.get("decision", "ask")}
    if r.get("reason"):
        out["reason"] = r["reason"]
    return out


def decide_here(payload):
    """No engine to ask (a CLI run, or the app is closed): decide with agy-loop's rules, and
    decline what would need a yes, since nobody is there to give it."""
    sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    from agyloop import policy
    call = payload.get("toolCall") or {}
    decision, reason, _ = policy.decide(os.environ.get("AGYLOOP_MODE", "default"), call.get("name", ""),
                                        call.get("args") or {})
    if decision == "ask":
        return {"decision": "deny", "reason": "This needs the user's approval, and agy-loop's app is not running "
                                              "to ask them. Continue without it, or explain what you need."}
    return {"decision": decision, **({"reason": reason} if reason else {})}


def pre_invocation(payload, sid):
    msgs = _call("POST", f"sessions/{sid}/steer", {}, timeout=10).get("messages") or []
    return {"injectSteps": [{"userMessage": m} for m in msgs]} if msgs else {}


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    kind = argv[0] if argv else ""
    sid = os.environ.get("AGYLOOP_SESSION")
    neutral = {"decision": "ask"} if kind == "pre-tool" else {}
    try:
        payload = json.loads(sys.stdin.read() or "{}")
    except ValueError:
        payload = {}
    out = neutral
    if sid:  # only turns agy-loop started; the verifier and your own agy are left alone
        try:
            out = pre_tool(payload, sid) if kind == "pre-tool" else pre_invocation(payload, sid)
        except Exception as e:  # engine gone or unreachable
            sys.stderr.write(f"agyloop hook: {e}\n")
            try:
                out = decide_here(payload) if kind == "pre-tool" else {}
            except Exception as e2:
                sys.stderr.write(f"agyloop hook: {e2}\n")
                out = {"decision": "deny", "reason": "agy-loop could not check this tool call."} if kind == "pre-tool" else {}
    sys.stdout.write(json.dumps(out))
    sys.stdout.flush()
    return 0


if __name__ == "__main__":
    sys.exit(main())
