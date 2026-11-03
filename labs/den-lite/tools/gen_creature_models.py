#!/usr/bin/env python3
"""Den Lite's creature models (src/creature_models.bin): creature entry -> its first model (display id), from
AzerothCore's world database `creature_template_model` (data/sql/base/db_world/creature_template_model.sql in
azerothcore-wotlk; the open project's own database, as tools/gen_objects.py reads its gameobject_template). Den Lite
needs it for one thing: a mount spell's aura names a creature entry, and the game shows the mount from the model the
server puts in UNIT_FIELD_MOUNTDISPLAYID (AzerothCore: ObjectMgr::ChooseDisplayId on the creature template).

Format: b"LDCM", u32 count, then per entry (sorted) a varint of the entry's difference to the last one and a varint
of the display id.

    python3 tools/gen_creature_models.py <creature_template_model.sql> src/creature_models.bin
"""
import re
import struct
import sys


def varint(n):
    out = bytearray()
    while True:
        b = n & 0x7F
        n >>= 7
        if n:
            out.append(b | 0x80)
        else:
            out.append(b)
            return bytes(out)


def main(src, dst):
    text = open(src, encoding="utf-8").read()
    first = {}
    for c, idx, d in re.findall(r"\((\d+),\s*(\d+),\s*(\d+),\s*[\d.]+,\s*[\d.]+,\s*(?:-?\d+|NULL)\)", text):
        c, idx, d = int(c), int(idx), int(d)
        if c not in first or idx < first[c][0]:
            first[c] = (idx, d)
    out = bytearray(b"LDCM")
    out += struct.pack("<I", len(first))
    last = 0
    for c in sorted(first):
        out += varint(c - last)
        out += varint(first[c][1])
        last = c
    open(dst, "wb").write(out)
    print(f"{len(first)} creatures, {len(out)} bytes")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
