#!/usr/bin/env node
/**
 * Builds every WASM engine, copies the binaries into frontend/public/engines/
 * and regenerates shared/data/engines.json (the single source of truth used by
 * both the frontend engine registry and the backend engine registry).
 *
 *   node scripts/build-engines.mjs            # cargo build + copy + manifest
 *   node scripts/build-engines.mjs --no-build # only copy + manifest
 *   node scripts/build-engines.mjs --check    # fail if the manifest is stale
 */
import { execSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const ENGINES_DIR = path.join(ROOT, "engines");
const TARGET = path.join(ENGINES_DIR, "target", "wasm32-unknown-unknown", "release");
const PUBLIC = path.join(ROOT, "frontend", "public", "engines");
const MANIFEST = path.join(ROOT, "shared", "data", "engines.json");
const TOOL_MAP = path.join(ROOT, "shared", "data", "engine-tools.json");

const args = new Set(process.argv.slice(2));

/** Engine metadata that cannot be derived from Cargo (names, capabilities). */
const ENGINES = [
  { id: "text-engine", crate: "text-engine", dir: "text", name: "Universal Text Processing Engine", capabilities: ["case-conversion", "text-analysis", "word-counting", "reading-time", "clean-lines", "json-format", "slug", "diff", "markdown", "unicode", "encodings", "morse", "lorem"] },
  { id: "pdf-engine", crate: "pdf-engine", dir: "pdf", name: "Client PDF Processing Engine", capabilities: ["pdf-inspection", "page-count", "encryption-check", "merge", "split", "rotate", "crop", "page-ops", "watermark", "stamp", "protect", "unlock", "metadata", "forms", "bookmarks", "redact", "compress", "repair", "extract-text", "to-html", "to-docx", "from-images", "from-text", "from-markdown", "from-html", "compare"] },
  { id: "image-engine", crate: "image-engine", dir: "image", name: "High-Performance Image Engine", capabilities: ["aspect-ratio", "grayscale-filter", "dimension-calc", "crop-calculation", "preset-crop", "resize", "crop", "convert", "compress", "adjust", "watermark", "metadata", "svg", "gif", "sprite", "favicon", "og-image", "palette"] },
  { id: "video-engine", crate: "video-engine", dir: "video", name: "Video Container Engine", capabilities: ["bitrate-calculation", "frame-calculation", "inspect", "mp4-lossless-trim", "mp4-mute", "mp4-concat", "subtitles", "gif-from-frames", "contact-sheet", "calculators"] },
  { id: "audio-engine", crate: "audio-engine", dir: "audio", name: "Audio Precision Engine", capabilities: ["file-size-calculation", "bpm-to-milliseconds", "decode", "wav-flac-encode", "trim", "join", "mix", "gain", "normalize", "fade", "stretch", "pitch", "eq", "denoise", "silence", "waveform", "spectrum", "tone"] },
  { id: "archive-engine", crate: "archive-engine", dir: "archive", name: "Archive Engine", capabilities: ["zip", "tar", "gzip", "list", "extract", "create", "test", "convert"] },
  { id: "json-engine", crate: "json-engine", dir: "json", name: "JSON / XML / YAML / TOML Engine", capabilities: ["json", "yaml", "xml", "toml", "ini", "plist", "csv", "jsonl", "schema", "diff", "code-generation"] },
  { id: "csv-engine", crate: "csv-engine", dir: "csv", name: "CSV Engine", capabilities: ["parse", "clean", "validate", "convert", "merge", "split", "dedupe", "filter", "sort", "sql", "statistics", "diff", "encoding"] },
  { id: "spreadsheet-engine", crate: "spreadsheet-engine", dir: "spreadsheet", name: "Spreadsheet Engine", capabilities: ["xlsx", "xls", "xlsb", "ods", "read", "write", "formulas", "evaluate", "diff", "merge", "split"] },
  { id: "crypto-engine", crate: "crypto-engine", dir: "crypto", name: "Cryptography Engine", capabilities: ["hash", "hmac", "bcrypt", "argon2", "passwords", "uuid", "ulid", "aes", "rsa", "jwt", "totp", "pem", "x509"] },
  { id: "encoding-engine", crate: "encoding-engine", dir: "encoding", name: "Encoding Engine", capabilities: ["base64", "base32", "hex", "url", "html-entities", "punycode", "quoted-printable", "mime-word", "msgpack", "cbor", "protobuf"] },
  { id: "developer-engine", crate: "developer-engine", dir: "developer", name: "Developer Tools Engine", capabilities: ["regex", "cron", "semver", "color", "sql", "graphql", "html", "css", "javascript", "linters", "gitignore", "env", "diff", "patch", "chmod", "mime", "http-status", "user-agent", "url", "query-string", "jsx"] },
  { id: "math-engine", crate: "math-engine", dir: "math", name: "Math Engine", capabilities: ["evaluate", "fractions", "statistics", "primes", "equations", "matrices", "trigonometry", "plot", "calculus", "bases", "roman", "sequences"] },
  { id: "units-engine", crate: "units-engine", dir: "units", name: "Unit Conversion Engine", capabilities: ["convert", "temperature", "cooking", "shoe-size", "paper-size", "pixels"] },
  { id: "finance-engine", crate: "finance-engine", dir: "finance", name: "Finance Engine", capabilities: ["loan", "mortgage", "interest", "investment", "deposits", "inflation", "cagr", "roi", "npv", "irr", "pricing", "salary", "currency", "tax-brackets"] },
  { id: "indiatax-engine", crate: "indiatax-engine", dir: "indiatax", name: "India GST & Tax Engine", capabilities: ["gst", "itc", "gstin", "hsn", "tds", "income-tax", "salary", "capital-gains", "bank-statement", "gstr", "reconcile"] },
  { id: "construction-engine", crate: "construction-engine", dir: "construction", name: "Construction Calculators Engine", capabilities: ["concrete", "masonry", "plaster", "paint", "tiles", "roofing", "steel", "rebar", "excavation", "staircase", "tank", "cost-estimate"] },
  { id: "solar-engine", crate: "solar-engine", dir: "solar", name: "Solar & Energy Engine", capabilities: ["panel-size", "roi", "battery", "inverter", "bill", "appliances", "carbon"] },
  { id: "datetime-engine", crate: "datetime-engine", dir: "datetime", name: "Date & Time Engine", capabilities: ["parse", "difference", "add", "age", "timezones", "timestamp", "business-days", "week-number", "countdown", "calendar", "meeting-planner", "cron", "pomodoro"] },
  { id: "business-engine", crate: "business-engine", dir: "business", name: "Business Calculators Engine", capabilities: ["documents", "profit-loss", "margin", "inventory", "pricing", "sales-tax", "commission", "payroll", "cash-flow", "break-even", "valuation", "unit-economics"] },
  { id: "qrbarcode-engine", crate: "qrbarcode-engine", dir: "qrbarcode", name: "QR & Barcode Engine", capabilities: ["qr-generate", "qr-decode", "qr-payloads", "bulk", "barcode-generate", "barcode-validate", "barcode-read"] },
  { id: "geo-engine", crate: "geo-engine", dir: "geo", name: "GIS / Geo Engine", capabilities: ["geojson", "kml", "wkt", "coordinates", "distance", "area", "simplify", "bbox", "geohash", "utm"] },
  { id: "colordesign-engine", crate: "colordesign-engine", dir: "colordesign", name: "Colour & Design Engine", capabilities: ["color-convert", "palette", "gradient", "contrast", "accessibility", "color-blindness", "image-palette", "css-generators", "type-scale", "favicon", "social-dimensions"] },
  { id: "dataviz-engine", crate: "dataviz-engine", dir: "dataviz", name: "Data Visualisation Engine", capabilities: ["svg-charts", "histogram", "frequency", "pivot", "profile", "describe", "outliers", "sample"] },
  { id: "font-engine", crate: "font-engine", dir: "font", name: "Font Engine", capabilities: ["ttf", "otf", "woff", "inspect", "char-map", "preview", "size-check", "pairing", "convert", "subset"] },
  { id: "mesh3d-engine", crate: "mesh3d-engine", dir: "mesh3d", name: "3D Mesh Engine", capabilities: ["stl", "obj", "gltf", "glb", "inspect", "convert", "transform", "repair", "thumbnail"] },
  { id: "document-engine", crate: "document-engine", dir: "document", name: "Document Engine", capabilities: ["docx", "pptx", "odt", "rtf", "epub", "mobi", "to-text", "to-html", "to-markdown", "template-fill", "from-markdown", "convert", "epub-validate", "epub-metadata"] },
  { id: "search-engine", crate: "dev-toolbox-search", dir: "search", name: "Fuzzy Search Engine", capabilities: ["fuzzy-search", "levenshtein-scoring", "category-filtering"], publicPath: "/search.wasm" },
];

function cargoVersion(dir) {
  const toml = fs.readFileSync(path.join(ENGINES_DIR, dir, "Cargo.toml"), "utf8");
  const m = toml.match(/^version\s*=\s*"([^"]+)"/m);
  return m ? m[1] : "1.0.0";
}

function sha256(buf) {
  return createHash("sha256").update(buf).digest("hex");
}

function build() {
  const crates = ENGINES.map((e) => `-p ${e.crate}`).join(" ");
  console.log("▶ cargo build --release --target wasm32-unknown-unknown …");
  execSync(`cargo build --release --target wasm32-unknown-unknown ${crates}`, { cwd: ENGINES_DIR, stdio: "inherit" });
}

function assemble() {
  const manifest = [];
  for (const e of ENGINES) {
    const wasmName = `${e.crate.replace(/-/g, "_")}.wasm`;
    const src = path.join(TARGET, wasmName);
    if (!fs.existsSync(src)) {
      console.warn(`⚠ missing ${src} – run without --no-build`);
      continue;
    }
    const version = cargoVersion(e.dir);
    const bytes = fs.readFileSync(src);
    let url;
    if (e.publicPath) {
      url = e.publicPath;
      fs.copyFileSync(src, path.join(ROOT, "frontend", "public", e.publicPath.replace(/^\//, "")));
    } else {
      const destDir = path.join(PUBLIC, e.dir, version);
      fs.mkdirSync(destDir, { recursive: true });
      fs.copyFileSync(src, path.join(destDir, wasmName));
      url = `/engines/${e.dir}/${version}/${wasmName}`;
    }
    manifest.push({
      id: e.id,
      name: e.name,
      version,
      type: "wasm",
      url,
      hash: sha256(bytes),
      size: bytes.length,
      capabilities: e.capabilities,
      dependencies: [],
    });
    console.log(`✔ ${e.id.padEnd(22)} v${version.padEnd(6)} ${(bytes.length / 1024 / 1024).toFixed(2).padStart(5)} MB  ${url}`);
  }
  // Attach tool counts from engine-tools.json when available.
  if (fs.existsSync(TOOL_MAP)) {
    const map = JSON.parse(fs.readFileSync(TOOL_MAP, "utf8"));
    for (const m of manifest) {
      const entry = map.engines.find((x) => x.id === m.id);
      m.tool_count = entry ? entry.tools.length : 0;
    }
  }
  return { schema_version: "1.0.0", generated_by: "scripts/build-engines.mjs", engines: manifest };
}

const doBuild = !args.has("--no-build") && !args.has("--check");
if (doBuild) build();
const manifest = assemble();
const json = JSON.stringify(manifest, null, 2) + "\n";
if (args.has("--check")) {
  const current = fs.existsSync(MANIFEST) ? fs.readFileSync(MANIFEST, "utf8") : "";
  if (current !== json) {
    console.error("✖ shared/data/engines.json is stale – run `node scripts/build-engines.mjs`");
    process.exit(1);
  }
  console.log("✔ engines.json is up to date");
} else {
  fs.writeFileSync(MANIFEST, json);
  console.log(`✔ wrote ${path.relative(ROOT, MANIFEST)} (${manifest.engines.length} engines)`);
}
