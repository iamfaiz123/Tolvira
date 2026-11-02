#!/usr/bin/env node
/**
 * End-to-end WASM integration tests. Loads every engine exactly like
 * `frontend/src/workers/engine.worker.ts` does (no imports except `env.abort`),
 * verifies the manifest hashes, checks that every action referenced by
 * shared/data/engine-tools.json exists, and runs valid / invalid / empty
 * cases per engine.
 *
 *   node scripts/test-engines.mjs            # all engines
 *   node scripts/test-engines.mjs csv-engine # one engine
 */
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifest = JSON.parse(fs.readFileSync(path.join(ROOT, "shared/data/engines.json"), "utf8"));
const toolMap = JSON.parse(fs.readFileSync(path.join(ROOT, "shared/data/engine-tools.json"), "utf8"));
const only = process.argv[2];

const b64 = (buf) => Buffer.from(buf).toString("base64");
const PNG = b64(fs.readFileSync(path.join(ROOT, "engines/image/tests/fixtures/sample.png")));
const TTF = b64(fs.readFileSync(path.join(ROOT, "engines/image/assets/DejaVuSans-subset.ttf")));
// 12-triangle unit cube as binary STL (header + count + 12 * 50 bytes), built in-process.
const STL = (() => {
  const V = [[0,0,0],[1,0,0],[1,1,0],[0,1,0],[0,0,1],[1,0,1],[1,1,1],[0,1,1]];
  const F = [[0,3,2],[0,2,1],[4,5,6],[4,6,7],[0,1,5],[0,5,4],[3,7,6],[3,6,2],[0,4,7],[0,7,3],[1,2,6],[1,6,5]];
  const buf = Buffer.alloc(84 + F.length * 50);
  buf.writeUInt32LE(F.length, 80);
  F.forEach((f, i) => { let o = 84 + i * 50 + 12; for (const vi of f) for (const c of V[vi]) { buf.writeFloatLE(c * 10, o); o += 4; } });
  return b64(buf);
})();
const MP3 = b64(fs.readFileSync(path.join(ROOT, "engines/audio/tests/fixtures/sample.mp3")));
const MP4 = b64(fs.readFileSync(path.join(ROOT, "engines/video/tests/fixtures/sample.mp4")));
const DOCX = b64(fs.readFileSync(path.join(ROOT, "engines/document/tests/fixtures/sample.docx")));
const entropy = b64(Buffer.from(Array.from({ length: 32 }, (_, i) => (i * 73 + 11) & 0xff)));

/** Per engine: cases = [action, payload, expectation] where expectation is
 *  "ok", an error code, or a function(result) => boolean. */
const CASES = {
  "text-engine": [["stats", { text: "Hello world. Two sentences!" }, (r) => r.words === 4 && r.sentences === 2], ["case", { text: "hello", target: "upper" }, "ok"], ["slug", { text: "" }, "E_EMPTY_INPUT"], ["rot", { text: "abc", shift: 99 }, "ok"]],
  "pdf-engine": [["from_text", { text: "Hello PDF" }, (r) => typeof r.output.file === "string"], ["inspect", { file: b64("not a pdf") }, (r) => r.valid === false], ["metadata", { file: b64("not a pdf") }, (r, e) => e !== undefined], ["merge", { files: [] }, "E_EMPTY_INPUT"]],
  "image-engine": [["info", { file: PNG }, (r) => r.width > 0], ["resize", { file: PNG, width: 16 }, (r) => typeof r.output.file === "string" && r.output.width === 16], ["info", { file: b64("nope") }, "E_UNSUPPORTED_FORMAT"], ["info", { file: "" }, "E_EMPTY_INPUT"]],
  "video-engine": [["inspect", { file: MP4, name: "s.mp4" }, (r) => r.container === "mp4"], ["calc_bitrate", { duration_seconds: 60, target_size_mb: 10 }, "ok"], ["inspect", { file: b64("garbage garbage garbage") }, "E_UNSUPPORTED_FORMAT"], ["inspect", { file: "" }, "E_EMPTY_INPUT"]],
  "audio-engine": [["inspect", { file: MP3, name: "s.mp3" }, (r) => r.codec === "mp3"], ["tone", { frequency: 440, seconds: 0.2 }, (r) => r.format === "wav"], ["inspect", { file: b64("garbage garbage garbage") }, (r, e) => e !== undefined], ["inspect", { file: "" }, "E_EMPTY_INPUT"]],
  "archive-engine": [["create", { files: [{ name: "a.txt", data: b64("hi") }], format: "zip" }, (r) => typeof r.output.file === "string"], ["list", { file: b64("not an archive") }, (r, e) => e !== undefined], ["list", { file: "" }, "E_EMPTY_INPUT"]],
  "json-engine": [["format", { text: '{"b":1,"a":[1,2]}' }, "ok"], ["validate", { text: "{bad" }, (r) => r.valid === false], ["format", { text: "" }, "E_EMPTY_INPUT"], ["to_yaml", { text: '{"a":1}' }, "ok"]],
  "csv-engine": [["parse", { text: "a,b\n1,2\n" }, (r) => r.total_rows === 1], ["to_sql", { text: "a\n1\n", dialect: "oracle" }, "E_INVALID_INPUT"], ["parse", { text: "" }, "E_EMPTY_INPUT"]],
  "spreadsheet-engine": [["from_csv", { text: "a,b\n1,2\n" }, (r) => typeof r.file === "string"], ["evaluate", { formula: "=SUM(1,2,3)" }, (r) => r.value === 6], ["inspect", { file: b64("PK\x03\x04junkjunkjunkjunk") }, "E_MALFORMED"], ["inspect", { file: "" }, "E_EMPTY_INPUT"]],
  "crypto-engine": [["hash", { text: "abc", algorithm: "sha256" }, (r) => JSON.stringify(r).includes("ba7816bf")], ["password", { length: 16, entropy }, "ok"], ["hmac", { text: "abc", key: "" , algorithm: "sha256" }, (r, e) => e !== undefined], ["hash", { text: "", algorithm: "sha256" }, "E_EMPTY_INPUT"]],
  "encoding-engine": [["base64_encode", { text: "hi" }, (r) => JSON.stringify(r).includes("aGk=")], ["base64_decode", { text: "***" }, (r, e) => e !== undefined], ["hex_decode", { text: "" }, "E_EMPTY_INPUT"]],
  "developer-engine": [["regex_test", { pattern: "\\d+", flags: "g", text: "a1b22" }, (r) => r.match_count === 2], ["sql_validate", { sql: "SELEC 1" }, (r) => r.valid === false], ["chmod", { mode: "999" }, "E_INVALID_INPUT"], ["chmod", { mode: "" }, "E_EMPTY_INPUT"]],
  "math-engine": [["evaluate", { expression: "2^10 + sqrt(16)" }, (r) => JSON.stringify(r).includes("1028")], ["evaluate", { expression: "1/0" }, (r, e) => e !== undefined || JSON.stringify(r).match(/inf|error/i)], ["evaluate", { expression: "" }, "E_EMPTY_INPUT"]],
  "units-engine": [["convert", { value: 1, from: "km", to: "m" }, (r) => JSON.stringify(r).includes("1000")], ["convert", { value: 1, from: "km", to: "kg" }, (r, e) => e !== undefined], ["convert", { value: 1, from: "", to: "m" }, (r, e) => e !== undefined]],
  "finance-engine": [["loan", { principal: 100000, annual_rate: 8, months: 12 }, "ok"], ["loan", { principal: -1, annual_rate: 8, months: 12 }, (r, e) => e !== undefined]],
  "indiatax-engine": [["gst", { amount: 1000, rate: 18 }, "ok"], ["validate_gstin", { gstin: "27AAPFU0939F1ZV" }, (r) => r.valid === true], ["validate_gstin", { gstin: "" }, (r, e) => e !== undefined || r.valid === false]],
  "construction-engine": [["concrete", { length: 1, width: 1, thickness: 1 }, (r) => r.wet_volume_m3 === 1], ["concrete", { length: 0, width: 1, thickness: 1 }, "E_INVALID_INPUT"]],
  "solar-engine": [["panel_size", { monthly_kwh: 300 }, (r) => r.panels >= 1], ["panel_size", {}, (r, e) => e !== undefined]],
  "datetime-engine": [["difference", { from: "2024-01-01", to: "2024-03-01" }, "ok"], ["difference", { from: "nope", to: "2024-03-01" }, (r, e) => e !== undefined]],
  "business-engine": [["break_even", { fixed_costs: 1000, price_per_unit: 10, variable_cost_per_unit: 5 }, (r) => r.break_even_units === 200], ["document", { items: [] }, "E_EMPTY_INPUT"]],
  "qrbarcode-engine": [["qr_generate", { data: "hello", formats: ["svg"] }, (r) => r.svg.startsWith("<svg")], ["barcode_validate", { code: "4006381333931" }, (r) => r.valid === true], ["qr_generate", { data: "" }, "E_EMPTY_INPUT"]],
  "geo-engine": [["distance", { from: "0,0", to: "0,1" }, (r) => Math.abs(r.distance.kilometers - 111.2) < 1], ["wkt_parse", { wkt: "BLOB (1 2)" }, "E_INVALID_INPUT"], ["format", {}, "E_EMPTY_INPUT"]],
  "colordesign-engine": [["convert", { color: "#336699" }, (r) => r.hsl.h === 210], ["palette", { color: "#fff", scheme: "nope" }, "E_INVALID_INPUT"], ["convert", { color: "" }, "E_EMPTY_INPUT"]],
  "font-engine": [["inspect", { file: TTF, name: "DejaVuSans.ttf" }, (r) => r.container === "ttf" && r.glyphs.count > 10], ["convert", { file: TTF, to: "woff" }, (r) => r.mime === "font/woff"], ["subset", { file: TTF, text: "Hello" }, (r) => r.bytes > 0], ["inspect", { file: b64("nope") }, "E_UNSUPPORTED_FORMAT"], ["convert", { file: TTF, to: "woff2" }, "E_UNSUPPORTED"], ["preview", {}, "E_EMPTY_INPUT"]],
  "mesh3d-engine": [["inspect", { file: STL, name: "cube.stl" }, (r) => r.triangles === 12 && r.watertight === true], ["convert", { file: STL, to: "obj" }, (r) => r.mime === "model/obj"], ["thumbnail", { file: STL, width: 32, height: 32 }, (r) => r.mime === "image/png"], ["inspect", { file: b64("nope") }, "E_UNSUPPORTED_FORMAT"], ["convert", { file: STL, to: "fbx" }, "E_INVALID_INPUT"], ["repair", {}, "E_EMPTY_INPUT"]],
  "document-engine": [["inspect", { file: DOCX, name: "s.docx" }, (r) => r.format === "docx"], ["from_markdown", { markdown: "# Hi\n\nthere" }, (r) => r.filename === "document.docx"], ["inspect", { file: b64("nope") }, "E_UNSUPPORTED_FORMAT"], ["to_text", {}, "E_EMPTY_INPUT"]],
  "dataviz-engine": [["chart", { csv: "k,v\na,1\nb,2\n" }, (r) => r.svg.startsWith("<svg")], ["chart", { csv: "k,v\na,1\n", spec: { kind: "radar" } }, "E_INVALID_INPUT"], ["chart", {}, "E_EMPTY_INPUT"]],
};

async function loadEngine(file) {
  const bytes = fs.readFileSync(file);
  const mod = await WebAssembly.compile(bytes);
  const imports = WebAssembly.Module.imports(mod);
  if (imports.length) throw new Error(`engine has imports: ${JSON.stringify(imports)}`);
  const inst = await WebAssembly.instantiate(mod, { env: { abort: () => {} } });
  const ex = inst.exports;
  for (const name of ["alloc", "dealloc", "execute", "result_len", "memory"]) {
    if (!(name in ex)) throw new Error(`missing export ${name}`);
  }
  return {
    bytes,
    run(action, payload) {
      const enc = new TextEncoder().encode(JSON.stringify({ action, payload }));
      const ptr = ex.alloc(enc.length);
      new Uint8Array(ex.memory.buffer).set(enc, ptr);
      const resPtr = ex.execute(ptr, enc.length);
      const len = ex.result_len();
      const out = new TextDecoder().decode(new Uint8Array(ex.memory.buffer, resPtr, len));
      ex.dealloc(ptr, enc.length);
      return JSON.parse(out);
    },
  };
}

let failures = 0;
let passed = 0;
const report = [];
function check(cond, label) {
  if (cond) passed++;
  else {
    failures++;
    console.error(`  ✖ ${label}`);
  }
}

for (const m of manifest.engines) {
  if (only && m.id !== only) continue;
  if (m.id === "search-engine") continue; // different protocol (search-specific exports)
  const file = path.join(ROOT, "frontend/public", m.url.replace(/^\//, ""));
  console.log(`\n${m.id} v${m.version}`);
  if (!fs.existsSync(file)) {
    check(false, `${m.url} missing`);
    continue;
  }
  let engine;
  try {
    engine = await loadEngine(file);
  } catch (e) {
    check(false, `load failed: ${e.message}`);
    continue;
  }
  check(createHash("sha256").update(engine.bytes).digest("hex") === m.hash, "manifest hash matches binary");
  check(engine.bytes.length === m.size, "manifest size matches binary");
  // Protocol: unknown action → E_BAD_REQUEST, garbage → error envelope.
  const bad = engine.run("__definitely_not_an_action__", {});
  check(bad.success === false && bad.error_code === "E_BAD_REQUEST", "unknown action → E_BAD_REQUEST");
  // Every mapped action must be recognised by the enum.
  const actions = [...new Set(toolMap.tools.filter((t) => t.engine === m.id && t.action).map((t) => t.action))];
  const unknown = actions.filter((a) => {
    const r = engine.run(a, {});
    return r.success === false && /unknown variant/.test(r.error || "");
  });
  check(unknown.length === 0, `all ${actions.length} mapped actions exist (unknown: ${unknown.join(", ")})`);
  // Functional cases.
  let ok = 0;
  for (const [action, payload, expect] of CASES[m.id] || []) {
    const t0 = performance.now();
    const r = engine.run(action, payload);
    const ms = (performance.now() - t0).toFixed(0);
    let pass;
    if (expect === "ok") pass = r.success === true;
    else if (typeof expect === "string") pass = r.success === false && r.error_code === expect;
    else pass = Boolean(expect(r.data, r.error_code));
    check(pass, `${action} ${JSON.stringify(payload).slice(0, 60)} → ${JSON.stringify(r).slice(0, 160)} (${ms} ms)`);
    if (pass) ok++;
  }
  console.log(`  ✔ ${ok}/${(CASES[m.id] || []).length} cases, ${actions.length} actions, ${(m.size / 1024 / 1024).toFixed(2)} MB`);
  report.push({ engine: m.id, version: m.version, size: m.size, actions: actions.length, cases: (CASES[m.id] || []).length, passed: ok });
}

console.log(`\n${passed} checks passed, ${failures} failed`);
if (process.env.ENGINE_REPORT) fs.writeFileSync(process.env.ENGINE_REPORT, JSON.stringify(report, null, 2));
process.exit(failures ? 1 : 0);
