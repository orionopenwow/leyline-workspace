// node-worker-shim.cjs — runs a browser worker script (solo/den-lite-worker.js) in a node worker thread for the solo
// tests: self, importScripts (relative to the script), postMessage/onmessage, and fetch for local files (the wasm).
'use strict';
const { parentPort, workerData } = require('node:worker_threads');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

let dir = path.dirname(workerData.script);
globalThis.self = globalThis;
globalThis.importScripts = (...urls) => {
  for (const u of urls) {
    const file = path.isAbsolute(u) ? u : path.resolve(dir, u);
    const was = dir;
    dir = path.dirname(file);   // a script's own importScripts are relative to it
    try { vm.runInThisContext(fs.readFileSync(file, 'utf8'), { filename: file }); } finally { dir = was; }
  }
};
globalThis.postMessage = (m, transfer) => parentPort.postMessage(m, transfer);
globalThis.fetch = async (u) => new Response(fs.readFileSync(String(u).replace(/^file:\/\//, '')), { headers: { 'content-type': 'application/wasm' } });
parentPort.on('message', (m) => { if (typeof globalThis.onmessage === 'function') globalThis.onmessage({ data: m }); });
importScripts(workerData.script);
