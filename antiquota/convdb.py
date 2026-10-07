"""What agy's stream-json leaves out, read from agy's own conversation database.

Each conversation is a SQLite file, <...>/antigravity-cli/conversations/<id>.db, whose
`steps` rows hold protobuf payloads. For a model reply (step_type 15) the planner
response is field 20: 20.1 is the reply text, 20.3 the model's thinking. agy's CLI
stream never carries the thinking, so the GUI reads it from here once a reply is done.
"""
import os
import sqlite3

AGENT_RESPONSE = 15


def _varint(b, i):
    x = shift = 0
    while True:
        c = b[i]
        i += 1
        x |= (c & 0x7F) << shift
        shift += 7
        if c < 0x80:
            return x, i


def _field(buf, number):
    """First length-delimited field `number` of a protobuf message, or None."""
    i = 0
    try:
        while i < len(buf):
            key, i = _varint(buf, i)
            f, wire = key >> 3, key & 7
            if wire == 0:
                _, i = _varint(buf, i)
            elif wire == 1:
                i += 8
            elif wire == 5:
                i += 4
            elif wire == 2:
                n, i = _varint(buf, i)
                if f == number:
                    return buf[i:i + n]
                i += n
            else:
                return None
    except IndexError:
        return None
    return None


def thinking_of(payload):
    planner = _field(payload or b"", 20)
    raw = _field(planner, 3) if planner else None
    try:
        return raw.decode("utf-8").strip() if raw else ""
    except UnicodeDecodeError:
        return ""


def thinking(conversations_dir, conversation_id, step_idx):
    """The thinking text of one reply step ('' if none or not written yet)."""
    path = os.path.join(conversations_dir, f"{conversation_id}.db")
    if not conversation_id or not os.path.exists(path):
        return ""
    try:
        con = sqlite3.connect(f"file:{path}?mode=ro", uri=True, timeout=2)
        try:
            row = con.execute("select step_type, step_payload from steps where idx = ?", (step_idx,)).fetchone()
        finally:
            con.close()
    except sqlite3.Error:
        return ""
    return thinking_of(row[1]) if row and row[0] == AGENT_RESPONSE else ""
