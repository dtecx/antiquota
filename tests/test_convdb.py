import sqlite3

from antiquota import convdb


def varint(n):
    out = bytearray()
    while True:
        out.append((n & 0x7F) | (0x80 if n > 0x7F else 0))
        n >>= 7
        if not n:
            return bytes(out)


def ld(field, payload):
    """A length-delimited protobuf field."""
    return varint(field << 3 | 2) + varint(len(payload)) + payload


def test_thinking_comes_from_field_20_3_of_a_reply(tmp_path):
    payload = ld(5, ld(12, b"meta")) + bytes([0x08, 0x96, 0x01]) + ld(20, ld(1, b"No") + ld(3, "Check 17... é".encode()))
    db = tmp_path / "conv.db"
    con = sqlite3.connect(db)
    con.execute("create table steps (idx integer, step_type integer, step_payload blob)")
    con.execute("insert into steps values (2, 15, ?)", (payload,))
    con.execute("insert into steps values (3, 14, ?)", (payload,))
    con.commit()
    con.close()
    assert convdb.thinking(str(tmp_path), "conv", 2) == "Check 17... é"
    assert convdb.thinking(str(tmp_path), "conv", 3) == ""   # not a reply step
    assert convdb.thinking(str(tmp_path), "missing", 2) == ""
    assert convdb.thinking_of(b"\xff\xff") == ""
