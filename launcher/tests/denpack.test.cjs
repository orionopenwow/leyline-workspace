// Den codes in the launcher (ui/denpack.js): the same format as Den Lite's (labs/den-lite/src/denpack.rs).
const test = require("node:test");
const assert = require("node:assert");
const p = require("../ui/denpack.js");

test("the zip CRC and base64url, one text one code", () => {
  assert.strictEqual(p.crc32(new TextEncoder().encode("123456789")), 0xCBF43926);
  assert.strictEqual(p.b64encode(new TextEncoder().encode("hello")), "aGVsbG8");
  assert.strictEqual(p.b64decode("aGVsbG9"), null, "the unused low bits must be zero");
  assert.strictEqual(p.b64decode("ab+c"), null);
});

test("a tale's code goes round, raw and deflated, through a chat's line breaks", async () => {
  const short = "tale Hi\nstep A\n goal talk jaina\n";
  // the very code Den Lite's denpack.rs makes for it (seal Raw)
  const rust = "ley1.tale.r.dGFsZSBIaQpzdGVwIEEKIGdvYWwgdGFsayBqYWluYQo.bdc22d4b";
  assert.strictEqual(await p.encode("tale", short), rust);
  const long = "tale Long\n" + "step A\n goal talk jaina\n".repeat(30);
  const z = await p.encode("tale", long);
  assert.match(z, /^ley1\.tale\.z\./);
  assert.strictEqual((await p.decode(`look: ${z.slice(0, 12)}\n${z.slice(12)} thanks!`)).text, long);
  assert.strictEqual((await p.decode(p.LINK + z)).text, long);
});

test("damaged, changed or oversized codes say why", async () => {
  const z = await p.encode("tale", "tale T\n" + "step A\n goal talk jaina\n".repeat(10));
  const changed = z.slice(0, -1) + (z.endsWith("0") ? "1" : "0");
  await assert.rejects(p.decode(changed), /changed or copied wrong/);
  await assert.rejects(p.decode("hello"), /isn't a Den code/);
  await assert.rejects(p.decode("ley1.map.r.AAAA.00000000"), /newer Leyline/);
  await assert.rejects(p.decode("ley1.den.r.A!AA.00000000"), /wrong letter|cut short/);
  // a deflate bomb stops at the kind's limit
  const big = "a".repeat(20000);
  const bomb = await p.encode("tale", big);
  await assert.rejects(p.decode(bomb), /too much/);
});

test("Sky Island's code is the island, and a tale becomes a small Den file Den Lite lists", async () => {
  const d = await p.decode(p.ISLAND_CODE);
  assert.strictEqual(d.kind, "den");
  const doc = JSON.parse(d.text);
  assert.strictEqual(doc.room.look, "sky");
  assert.ok(doc.tales[0].startsWith("tale Welcome to Sky Island"));
  assert.ok((p.LINK + p.ISLAND_CODE).length < 2000, "a link");
  const f = p.asDenFile("tale", "tale The Lost Lantern\nstep A\n goal talk jaina\n");
  assert.strictEqual(f.fileName, "tale-the-lost-lantern.den.json");
  assert.deepStrictEqual(JSON.parse(f.text).tales.length, 1);
  assert.strictEqual(p.asDenFile("den", d.text).fileName, "sky-island.den.json");
});
