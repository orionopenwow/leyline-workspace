// Den codes in the launcher (docs/DEN-NEXT.md, "Passing a Den around"): a whole Den, a tale or a stamp as one short
// text a player can paste, put in a link (leyline://pack/<code>) or keep as a small .denpack file. The format is Den
// Lite's (labs/den-lite/src/denpack.rs): ley1.<den|tale|stamp>.<r|z>.<base64url>.<crc32 of the text>, z = raw deflate.
// Nothing in a code runs: what it holds is checked by the core (den_add: a Den file's checks) and by Den Lite again
// (den.rs, tale.rs) before anything opens. No server: the code IS the Den.
(function (root) {
  "use strict";
  const MAX_CODE = 96 * 1024;
  const MAX_TEXT = { den: 512 * 1024, tale: 8 * 1024, stamp: 64 * 1024 };
  // Sky Island (labs/den-lite/src/skyisland.rs den_json("sky-island")): a Den Lite test checks this decodes to it
  const ISLAND_CODE = "ley1.den.z.XVNLb9swDP4r33xpC6SGh60P5DYU27Bh26UFimHpQYkZW7UsGpLczC3630eqSZb2ZJIyvwcpPRVrDr1JxRyFo8lZT6c1-WKG4oFCtOzl5L1k3vSkP8VuOrXRGV_rP8kml8vX3YRv-7LCaPUT1o5Nsr7BS88cy9G6eoY4mDDDYGKETWBfaltg7qXtqXDM3ZZM69E-Ktz5x2dJhsBDlOzPnSbOrCi39GbYKv2r37OLqpJ4OogfNb6sDry8Fm0CGalWSrJ1qMCdzcFOSzC1HZX_PP-YjKOsJke4JbfinpAY_8EXfjnhu7HeLLxZ8pggc7EhJmyM6xB49DUmHsPbaZULH23jQXVDMAkVTs8qLIorkboe3RypJYgu-R0GjqVxYybUvPEljn_zeOQcFAhLs-ogKrQhBaLyZFEIeKIB19LRknO88ECU5F6VCsuBlyzuRdM7XGmxpSCS0EvsU6lgQMPGIaml-xezGf-Gx1WbidnVmfwt0We5aVNq1XeGbQJvItaBe2kT21kwvjLUScpw9hWnLE5qFS5xvjMl10tHYpPcz7d8N6KlHvveUjycCVbGi_xOfbVCgOuWN9ABtHraMNdyUSkeMte0JlH4YYenB2Ik4caEhpIg6cGUlZuUdAvHHIRJli99Np2UW8U_5MqDZRJZj258T_Jgo7yRXakWDXsrv0ShTUcxbyjOMQSSF_XlQpeWX1qJn-ooyp7yhGcK7w8fnroJtDFBZquvOV_cW9kihf3BqiXNirvnfw.4d5caff2";

  const B64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
  function b64encode(bytes) {
    let out = "";
    for (let i = 0; i < bytes.length; i += 3) {
      const n = (bytes[i] << 16) | ((bytes[i + 1] || 0) << 8) | (bytes[i + 2] || 0);
      const k = Math.min(3, bytes.length - i) + 1;
      for (let j = 0; j < k; j++) out += B64[(n >> (18 - 6 * j)) & 63];
    }
    return out;
  }
  function b64decode(s) {
    if (s.length % 4 === 1 || /[^A-Za-z0-9_-]/.test(s)) return null;
    const out = [];
    for (let i = 0; i < s.length; i += 4) {
      const c = s.slice(i, i + 4);
      let n = 0;
      for (let j = 0; j < c.length; j++) n |= B64.indexOf(c[j]) << (18 - 6 * j);
      out.push((n >> 16) & 255);
      if (c.length > 2) out.push((n >> 8) & 255);
      if (c.length > 3) out.push(n & 255);
    }
    const bytes = Uint8Array.from(out);
    return b64encode(bytes) === s ? bytes : null;   // one text, one code (as Den Lite)
  }
  let table = null;
  function crc32(bytes) {
    if (!table) {
      table = new Uint32Array(256);
      for (let i = 0; i < 256; i++) { let c = i; for (let k = 0; k < 8; k++) c = c & 1 ? (c >>> 1) ^ 0xEDB88320 : c >>> 1; table[i] = c >>> 0; }
    }
    let c = 0xFFFFFFFF;
    for (const b of bytes) c = table[(c ^ b) & 255] ^ (c >>> 8);
    return (c ^ 0xFFFFFFFF) >>> 0;
  }
  async function stream(bytes, how, max) {
    const s = new Blob([bytes]).stream().pipeThrough(how);
    const reader = s.getReader();
    const parts = [];
    let n = 0;
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      n += value.length;
      if (max && n > max) { try { await reader.cancel(); } catch (_) { /* gone */ } return null; }
      parts.push(value);
    }
    const out = new Uint8Array(n);
    let at = 0;
    for (const p of parts) { out.set(p, at); at += p.length; }
    return out;
  }
  // the code in a pasted text: bare, a leyline://pack/ link, or broken over lines by a chat
  function find(text) {
    let s = String(text || "").trim().replace(/^leyline:\/\/pack\//i, "");
    const i = s.indexOf("ley1.");
    if (i < 0) return null;
    s = s.slice(i).replace(/\s+/g, "");
    const m = /^ley1\.[a-z]+\.[a-z]\.[A-Za-z0-9_-]*\.[0-9a-f]{8}/.exec(s);
    return m ? m[0] : s.split(/[^A-Za-z0-9._-]/)[0];
  }
  // { kind, text } or throws a plain-words Error
  async function decode(pasted) {
    const code = find(pasted);
    if (!code) throw new Error("That isn't a Den code (they start with ley1.).");
    if (code.length > MAX_CODE) throw new Error("That code is too long.");
    const p = code.split(".");
    if (p.length !== 5 || p[0] !== "ley1") throw new Error("That Den code is cut short or from a newer Leyline.");
    const kind = p[1];
    if (!(kind in MAX_TEXT) || (p[2] !== "r" && p[2] !== "z")) throw new Error("That Den code is from a newer Leyline.");
    const body = b64decode(p[3]);
    if (!body) throw new Error("That Den code has a wrong letter in it (copied whole?).");
    if (!/^[0-9a-f]{8}$/.test(p[4])) throw new Error("That Den code's end is missing (copied whole?).");
    const bytes = p[2] === "r" ? body : await stream(body, new DecompressionStream("deflate-raw"), MAX_TEXT[kind]).catch(() => null);
    if (!bytes || bytes.length > MAX_TEXT[kind]) throw new Error("That Den code holds too much, or is damaged.");
    if (crc32(bytes) !== parseInt(p[4], 16)) throw new Error("That Den code was changed or copied wrong (its check doesn't match).");
    return { kind, text: new TextDecoder("utf-8", { fatal: true }).decode(bytes) };
  }
  async function encode(kind, text) {
    const bytes = new TextEncoder().encode(text);
    const z = await stream(bytes, new CompressionStream("deflate-raw"));
    const hex = crc32(bytes).toString(16).padStart(8, "0");
    return z && z.length < bytes.length ? `ley1.${kind}.z.${b64encode(z)}.${hex}` : `ley1.${kind}.r.${b64encode(bytes)}.${hex}`;
  }
  const slug = (s) => String(s || "").toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 19) || "den";
  // what the core's den_add takes: a Den file (a tale goes in a small Den file of its own, "tale-<title>", which Den
  // Lite's .den tale list reads)
  function asDenFile(kind, text) {
    if (kind === "tale") {
      const title = (/^\s*tale\s+(.+)$/m.exec(text) || [])[1] || "tale";
      const name = "tale-" + slug(title);
      return { fileName: name + ".den.json", text: JSON.stringify({ format: "leyline-den", version: 1, name, title: title.slice(0, 48), props: [], tales: [text] }) };
    }
    let name = "den";
    try { name = slug(JSON.parse(text).name || "den"); } catch (_) { /* the core says what is wrong */ }
    return { fileName: name + ".den.json", text };
  }
  const api = { decode, encode, find, asDenFile, crc32, b64encode, b64decode, ISLAND_CODE, LINK: "leyline://pack/" };
  if (typeof module !== "undefined" && module.exports) module.exports = api;
  root.LeylineDenPack = api;
})(typeof window !== "undefined" ? window : globalThis);
