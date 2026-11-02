/**
 * End-to-end check of the generic engine UI without a browser.
 *
 * For every engine action the form schema (`shared/data/engine-schemas.json`)
 * is instantiated exactly like `EngineToolRunner` does, filled with sample
 * values (fixture files, sample text, numbers…), converted into a payload by
 * `buildPayload` and sent to the real `.wasm` engine. The engine must accept
 * the *shape* of the payload (anything but `E_BAD_REQUEST`), which proves that
 * the generated form matches the Rust action enums. Tool presets are verified
 * to survive the form round-trip, and a few tools are server-rendered with
 * React to make sure the real inputs (file pickers, keypad, switches…) are in
 * the markup.
 *
 * Run:  pnpm --filter @workspace/frontend run test:engine-ui [engine-id]
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import React from "react";
import { renderToString } from "react-dom/server";

import { getActionSchema, listActions, type FieldSchema } from "../src/registry/engine-schemas";
import { buildPayload, initialValues, partitionFields, presetToForm, verbFor, type FormValues } from "../src/tools/engine/form-model";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const only = process.argv[2];

// ---------------------------------------------------------------- polyfills
// `buildPayload` uses FileReader to base64-encode uploads; Node has File/Blob
// but no FileReader.
class NodeFileReader {
  result: string | null = null;
  error: Error | null = null;
  onload: (() => void) | null = null;
  onerror: (() => void) | null = null;
  readAsDataURL(file: File) {
    file.arrayBuffer().then((buf) => {
      this.result = `data:${file.type || "application/octet-stream"};base64,${Buffer.from(buf).toString("base64")}`;
      this.onload?.();
    });
  }
}
(globalThis as any).FileReader = NodeFileReader;
(globalThis as any).crypto ??= (await import("node:crypto")).webcrypto;

// ----------------------------------------------------------------- fixtures
const read = (p: string) => fs.readFileSync(path.join(ROOT, p));
const fixtureFile = (p: string, type: string) => new File([read(p)], path.basename(p), { type });
const PNG = fixtureFile("engines/image/tests/fixtures/sample.png", "image/png");
const MP3 = fixtureFile("engines/audio/tests/fixtures/sample.mp3", "audio/mpeg");
const MP4 = fixtureFile("engines/video/tests/fixtures/sample.mp4", "video/mp4");
const DOCX = fixtureFile("engines/document/tests/fixtures/sample.docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document");
const TTF = fixtureFile("engines/image/assets/DejaVuSans-subset.ttf", "font/ttf");
const CSV = new File(["name,qty,price\napple,3,1.5\npear,5,2.25\n"], "sample.csv", { type: "text/csv" });
const JSONF = new File([JSON.stringify({ asset: { version: "2.0" }, meshes: [] })], "sample.json", { type: "application/json" });

function cubeStl(): File {
  const V = [[0, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 0], [0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]];
  const F = [[0, 3, 2], [0, 2, 1], [4, 5, 6], [4, 6, 7], [0, 1, 5], [0, 5, 4], [3, 7, 6], [3, 6, 2], [0, 4, 7], [0, 7, 3], [1, 2, 6], [1, 6, 5]];
  const buf = Buffer.alloc(84 + F.length * 50);
  buf.writeUInt32LE(F.length, 80);
  F.forEach((f, i) => {
    let o = 84 + i * 50 + 12;
    for (const vi of f) for (const c of V[vi]) {
      buf.writeFloatLE(c * 10, o);
      o += 4;
    }
  });
  return new File([buf], "cube.stl", { type: "model/stl" });
}

// ------------------------------------------------------------------ engines
interface Engine {
  run(action: string, payload: unknown): any;
}

async function loadEngine(file: string): Promise<Engine> {
  const bytes = fs.readFileSync(file);
  const mod = await WebAssembly.compile(bytes);
  const inst = await WebAssembly.instantiate(mod, { env: { abort: () => {} } });
  const ex = inst.exports as any;
  return {
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

const manifest = JSON.parse(fs.readFileSync(path.join(ROOT, "shared/data/engines.json"), "utf8"));
const toolMap = JSON.parse(fs.readFileSync(path.join(ROOT, "shared/data/engine-tools.json"), "utf8"));
const engines = new Map<string, Engine>();
for (const m of manifest.engines) {
  if (m.id === "search-engine") continue;
  if (only && m.id !== only) continue;
  const file = path.join(ROOT, "frontend/public", m.url.replace(/^\//, ""));
  engines.set(m.id, await loadEngine(file));
}

// Generated fixtures for formats we do not ship as files.
const generated: Record<string, File> = {};
if (engines.has("pdf-engine")) {
  const r = engines.get("pdf-engine")!.run("from_text", { text: "Hello PDF\n\nSecond paragraph.", options: {} });
  generated.pdf = new File([Buffer.from(r.data.output.file, "base64")], "sample.pdf", { type: "application/pdf" });
}
if (engines.has("archive-engine")) {
  const r = engines.get("archive-engine")!.run("create", { files: [{ name: "a.txt", data: Buffer.from("hello").toString("base64") }], format: "zip" });
  generated.zip = new File([Buffer.from(r.data.output.file, "base64")], "sample.zip", { type: "application/zip" });
}
if (engines.has("spreadsheet-engine")) {
  const r = engines.get("spreadsheet-engine")!.run("from_csv", { text: "a,b\n1,2\n", name: "sample" });
  generated.xlsx = new File([Buffer.from(r.data.file, "base64")], "sample.xlsx", { type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" });
}
if (engines.has("qrbarcode-engine")) {
  const r = engines.get("qrbarcode-engine")!.run("qr_generate", { data: "hello", formats: ["png"] });
  generated.qr = new File([Buffer.from(r.data.png, "base64")], "qr.png", { type: "image/png" });
}

function sampleFile(engineId: string, field: FieldSchema): File {
  const accept = field.accept ?? "";
  if (accept.startsWith("image/")) return engineId === "qrbarcode-engine" && generated.qr ? generated.qr : PNG;
  if (accept.includes(".pdf")) return generated.pdf ?? PNG;
  if (accept.includes(".mp3")) return MP3;
  if (accept.includes(".mp4")) return MP4;
  if (accept.includes(".zip")) return generated.zip ?? PNG;
  if (accept.includes(".xlsx")) return generated.xlsx ?? CSV;
  if (accept.includes(".docx")) return DOCX;
  if (accept.includes(".ttf")) return TTF;
  if (accept.includes(".stl")) return cubeStl();
  if (accept.includes(".csv")) return CSV;
  if (accept.includes(".geojson") || accept.includes(".json")) return JSONF;
  return CSV;
}

const SAMPLE_TEXT: Record<string, string> = {
  text: "Hello world. This is sample text for the engine UI test.",
  html: "<h1>Title</h1><p>Hello <b>world</b></p>",
  markdown: "# Title\n\nHello **world**",
  sql: "SELECT id, name FROM users WHERE id = 1;",
  css: "a { color: red; }",
  json: '{"a": 1, "b": [1, 2]}',
  yaml: "a: 1\nb: [1, 2]\n",
  xml: "<root><a>1</a></root>",
  csv: "name,qty\napple,3\npear,5\n",
  expression: "2 + 3 * 4",
  wkt: "POINT (1 2)",
  kml: '<?xml version="1.0"?><kml xmlns="http://www.opengis.net/kml/2.2"><Placemark><Point><coordinates>1,2</coordinates></Point></Placemark></kml>',
  url: "https://example.com/path?a=1",
  color: "#336699",
  from: "2024-01-01",
  to: "2024-03-01",
  date: "2024-01-01",
  now: "2024-01-01T00:00:00Z",
  pattern: "[a-z]+",
  cron: "*/5 * * * *",
  expression_cron: "*/5 * * * *",
  version: "1.2.3",
  code: "4006381333931",
  data: "hello",
  password: "secret",
  secret: "JBSWY3DPEHPK3PXP",
  gstin: "27AAPFU0939F1ZV",
  number: "INV-001",
  equation: "3x + 2 = 11",
  wave: "sine",
  digits: "123",
  coordinate: "12.9716, 77.5946",
  hash: "ezp6mxhbyiq",
  ulid: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
  token: "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxIn0.abc",
};

function sampleValue(engineId: string, f: FieldSchema, siblings: FieldSchema[]): unknown {
  switch (f.kind) {
    case "file":
      return sampleFile(engineId, f);
    case "files":
      return [sampleFile(engineId, f), sampleFile(engineId, f)];
    case "text":
    case "textarea":
      return SAMPLE_TEXT[f.name] ?? (f.name === "name" ? "sample" : f.options?.[0]?.value ?? "sample");
    case "select":
      return f.options?.[0]?.value ?? "";
    case "color":
      return "#336699";
    case "number":
    case "integer":
      return f.min !== undefined ? f.min : 1;
    case "boolean":
      return false;
    case "tags":
      return f.suggestions?.slice(0, 2).join(", ") ?? "alpha, beta";
    case "numbers":
      return f.fixed ? Array.from({ length: f.fixed }, (_, i) => i + 1).join(", ") : "1, 2, 3";
    case "matrix":
      return "1, 2\n3, 4";
    case "json":
      return f.array ? "[]" : "{}";
    case "keyvalues":
      return [{ key: "a", value: "1" }];
    case "group":
      return fill(engineId, f.fields ?? [], initialValues(f.fields ?? []), true);
    case "list": {
      const item = f.item!;
      if (item.kind === "variant") {
        const v = item.variants![0];
        return [{ [item.tag ?? "op"]: v.value, ...fill(engineId, v.fields ?? [], initialValues(v.fields ?? []), true) }];
      }
      return [fill(engineId, item.fields ?? [], initialValues(item.fields ?? []), true)];
    }
    case "variant": {
      const v = f.variants![0];
      return { [f.tag ?? "op"]: v.value, ...fill(engineId, v.fields ?? [], initialValues(v.fields ?? []), true) };
    }
    default:
      return "sample";
  }
}

/** Fills required fields (and optionally all fields) with sample values. */
function fill(engineId: string, fields: FieldSchema[], values: FormValues, everything: boolean): FormValues {
  const out = { ...values };
  for (const f of fields) {
    if (f.auto) continue;
    if (f.required || everything) out[f.name] = sampleValue(engineId, f, fields);
  }
  return out;
}

// -------------------------------------------------------------------- tests
let passed = 0;
let failed = 0;
function check(cond: boolean, label: string) {
  if (cond) passed++;
  else {
    failed++;
    console.error(`  ✖ ${label}`);
  }
}

for (const [engineId, engine] of engines) {
  console.log(`\n${engineId}`);
  const actions = listActions(engineId);
  check(actions.length > 0, "schema has actions");
  let accepted = 0;
  for (const action of actions) {
    const schema = getActionSchema(engineId, action)!;
    for (const everything of [false, true]) {
      const values = fill(engineId, schema.fields, initialValues(schema.fields), everything);
      const { payload, problems } = await buildPayload(schema.fields, values);
      check(problems.length === 0, `${action}: form validation passes (${problems.map((p) => `${p.path}: ${p.message}`).join("; ")})`);
      if (process.env.TRACE) console.log(`  → ${action}${everything ? " (all)" : ""}`);
      const r = engine.run(action, payload);
      const shapeOk = r.success === true || (r.error_code !== "E_BAD_REQUEST" && !/unknown field|missing field `|invalid type/.test(r.error || ""));
      check(shapeOk, `${action}${everything ? " (all fields)" : ""}: engine accepts the generated payload shape – ${r.error_code}: ${(r.error || "").slice(0, 120)}`);
      if (r.success) accepted++;
    }
  }
  console.log(`  ${actions.length} actions, ${accepted} sample runs succeeded outright`);
}

// Presets (catalog tools and registry aliases) round-trip through the form.
let presets = 0;
const presetCases: [string, string, string, Record<string, unknown>][] = [];
for (const t of toolMap.tools) {
  if (!t.engine || (only && t.engine !== only)) continue;
  if (t.preset) presetCases.push([t.id, t.engine, t.action, t.preset]);
  for (const [slug, p] of Object.entries(t.slug_presets ?? {})) presetCases.push([`${t.id} (alias ${slug})`, t.engine, t.action, p as Record<string, unknown>]);
}
for (const [id, engineId, action, preset] of presetCases) {
  const schema = getActionSchema(engineId, action);
  check(Boolean(schema), `${id}: schema for preset action ${action}`);
  if (!schema) continue;
  const values = initialValues(schema.fields, preset);
  const filled = fill(engineId, schema.fields, values, false);
  const { payload, problems } = await buildPayload(schema.fields, filled);
  check(problems.length === 0, `${id}: preset form validates (${problems.map((p) => p.message).join("; ")})`);
  for (const [k, v] of Object.entries(preset)) {
    const f = schema.fields.find((x) => x.name === k)!;
    const expected = JSON.stringify(v);
    const got = JSON.stringify(payload[k]);
    const ok = got === expected || JSON.stringify(presetToForm(f, v)) === JSON.stringify(values[k]);
    check(ok, `${id}: preset ${k} survives the form (${got} vs ${expected})`);
  }
  const r = engines.get(engineId)?.run(action, payload);
  if (r) check(r.error_code !== "E_BAD_REQUEST", `${id}: engine accepts the preset payload (${r.error})`);
  presets++;
}
console.log(`\n${presets} tool presets verified`);

// Server-side render a few tools to make sure real inputs are produced.
if (!only) {
  const { EngineToolRunner } = await import("../src/tools/engine/EngineToolRunner");
  const { findToolBySlug } = await import("../src/registry/tool-registry");
  const expectations: [string, RegExp[]][] = [
    ["merge-pdf", [/type="file"/, /multiple/, /\.pdf/, /Merge|Run|Convert/]],
    ["image-resizer", [/type="file"/, /image\/\*/, /Width/, /Height/, /type="number"/]],
    ["scientific-calculator", [/data-testid="keypad"/, /sin/, /Calculate/]],
    ["loan-calculator", [/Principal/, /type="number"/, /Calculate/]],
    ["md5-generator", [/textarea/, /type="file"/]],
    ["qr-code-generator", [/textarea|input/, /Generate/]],
  ];
  for (const [slug, patterns] of expectations) {
    const tool = findToolBySlug(slug);
    check(Boolean(tool), `tool ${slug} exists in the registry`);
    if (!tool) continue;
    const html = renderToString(React.createElement(EngineToolRunner, { tool }));
    check(!/base64 encoded|JSON payload|payload\.file/i.test(html), `${slug}: no base64 / JSON payload inputs in the UI`);
    for (const p of patterns) check(p.test(html), `${slug}: markup matches ${p}`);
  }
  // Result rendering: real engine output through ResultView.
  const { ResultView } = await import("../src/tools/engine/ResultView");
  const renderResult = (engineId: string, action: string, payload: unknown) => {
    const r = engines.get(engineId)!.run(action, payload);
    check(r.success === true, `${engineId}.${action} sample run succeeds (${r.error})`);
    return renderToString(React.createElement(ResultView, { data: r.data }));
  };
  const loan = renderResult("finance-engine", "loan", { principal: 100000, annual_rate: 8, years: 5 });
  check(/Monthly payment|payment/i.test(loan) && /data-testid="output-stats"/.test(loan), "loan result renders stat cards");
  const pngB64 = Buffer.from(await PNG.arrayBuffer()).toString("base64");
  const resized = renderResult("image-engine", "resize", { file: pngB64, width: 16 });
  check(/Download/.test(resized) && /data-testid="output-file"/.test(resized) && !/[A-Za-z0-9+/]{200,}/.test(resized.replace(/src="[^"]*"/g, "")), "image result renders a download card without dumping base64");
  const qr = renderResult("qrbarcode-engine", "qr_generate", { data: "hello", formats: ["svg"] });
  check(/<svg/.test(qr) && /Preview|Source/.test(qr), "QR result renders the SVG preview");
  const stats = renderResult("text-engine", "stats", { text: "Hello world. Another sentence here." });
  check(/data-testid="output-stats"/.test(stats), "text stats render as stat cards");
  const table = renderResult("csv-engine", "parse", { text: "a,b\n1,2\n3,4\n" });
  check(/<table/.test(table) || /output-stats/.test(table), "CSV parse renders a table");

  // Layout sanity: every implemented tool has at least one visible input.
  let noInputs = 0;
  for (const t of toolMap.tools) {
    if (!t.engine || !t.action || !["implemented", "partial"].includes(t.status)) continue;
    const schema = getActionSchema(t.engine, t.action);
    if (!schema) {
      noInputs++;
      continue;
    }
    const { primary, options } = partitionFields(schema.fields, t.preset);
    if (primary.length + options.length === 0 && schema.fields.length > 0) noInputs++;
  }
  check(noInputs === 0, `every implemented tool renders inputs (${noInputs} without)`);
  const { ALL_TOOLS } = await import("../src/registry/tool-registry");
  const available = ALL_TOOLS.filter((t) => t.status === "available").length;
  console.log(`${available} of ${ALL_TOOLS.length} registry tools are available in the UI`);
  check(available > 600, "most registry tools are available");
  check(verbFor("convert") === "Convert" && verbFor("loan") === "Calculate" && verbFor("qr_generate") === "Generate", "run-button verbs");
}

console.log(`\n${passed} checks passed, ${failed} failed`);
process.exit(failed ? 1 : 0);
