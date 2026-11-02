# Dev Toolbox WASM Engines

Rust crates compiled to `wasm32-unknown-unknown` that implement the tool
catalog (`catalog.json`). One engine serves many tools; there is never one
binary per utility. Every engine speaks the same protocol, so the frontend
worker (`frontend/src/workers/engine.worker.ts`) and the backend registry need
no per-engine code.

## Protocol

```
request  = { "action": "<snake_case>", "payload": { ... } }   (UTF-8 JSON)
response = { "success": true,  "data": ... }
         | { "success": false, "error": "<human message>", "error_code": "E_…" }
```

* Exports: `alloc(len) -> ptr`, `dealloc(ptr, len)`, `execute(ptr, len) -> ptr`,
  `result_len() -> usize`, `memory`. **No imports** (only an unused `env.abort`
  may be provided). Anything that needs the OS (clock, randomness, files) is
  passed in by the caller instead.
* Files travel as base64 strings (`file`, `files[].file`, `image`, …). Results
  that are files come back as `{ file, filename, mime, bytes }` (or nested
  under `output` in the legacy text/pdf/image/archive envelopes).
* Randomness: pass `payload.entropy` (≥ 16 random bytes, base64). Engines that
  require it return `E_ENTROPY_REQUIRED` otherwise (crypto password / UUID v4 /
  RSA).
* Time: pass `now` / `today_serial` / `from` explicitly (datetime, cron,
  spreadsheet `TODAY()`).

### Error codes (`engine-shared::error::ErrorCode`)

| code | meaning |
|---|---|
| `E_BAD_REQUEST` | unknown action or payload does not match the action schema |
| `E_EMPTY_INPUT` | required text/file/list is empty |
| `E_INVALID_INPUT` | values out of range, bad options, unparsable references |
| `E_UNSUPPORTED_FORMAT` | container/codec/encoding not recognised |
| `E_MALFORMED` | recognised format but corrupt/truncated |
| `E_ENCRYPTED` / `E_WRONG_PASSWORD` | protected documents |
| `E_LIMIT_EXCEEDED` | size/count guard hit (`engine-shared::limits`) |
| `E_NOT_FOUND` | sheet/column/code/track not found |
| `E_UNSUPPORTED` | action exists but cannot run in WASM; message names the right runtime |
| `E_ENTROPY_REQUIRED` | caller must provide `entropy` |
| `E_INTERNAL` | unexpected failure (bug) |

## Workspace layout

| crate | type | serves catalog engine(s) | notes |
|---|---|---|---|
| `shared` (`engine-shared`) | rlib | all | protocol, errors, base64 adaptors, ChaCha20 RNG pool, limits, `export_engine!` macro |
| `tabular` (`tabular-core`) | rlib | csv, spreadsheet, dataviz, geo | CSV model, cleaning, validation, transforms, encodings |
| `colorcore` (`color-core`) | rlib | developer, color_design | colour maths |
| `text` | cdylib | text | v2 – 38 actions |
| `pdf` | cdylib | pdf | v2 – lopdf based, 45 actions |
| `image` | cdylib | image | v2 – image/resvg/ab_glyph, 39 actions |
| `video` | cdylib | video | v2 – MP4/MKV inspection, lossless MP4 edits, subtitles, GIF assembly |
| `audio` | cdylib | audio | v2 – symphonia decode, WAV/FLAC encode, DSP |
| `archive` | cdylib | archive | zip/tar/gzip |
| `json` | cdylib | json, xml_yaml_toml | JSON/YAML/XML/TOML/INI/plist/CSV/JSONL |
| `csv` | cdylib | csv | thin wrapper over `tabular-core` |
| `spreadsheet` | cdylib | spreadsheet | calamine reader, own XLSX/ODS writer, formula evaluator |
| `crypto` | cdylib | crypto | hashes, KDFs, AES/ChaCha, RSA, JWT, TOTP, X.509 |
| `encoding` | cdylib | encoding | base64/32, hex, URL, HTML, punycode, QP, MIME words, msgpack, CBOR, protobuf |
| `developer` | cdylib | developer | regex, cron, semver, SQL, GraphQL, HTML/CSS/JS, linters, diff/patch, lookups |
| `math`, `units`, `finance`, `indiatax`, `construction`, `solar`, `datetime`, `business` | cdylib | calculators | pure maths, caller-supplied rates/brackets/clock |
| `qrbarcode` | cdylib | qr_barcode, barcode_labels | QR (qrcode/rqrr), 1-D barcodes, GTIN validation |
| `geo` | cdylib | gis_geo | GeoJSON/KML/WKT, geodesy, UTM, geohash |
| `colordesign` | cdylib | color_design | palettes, gradients, WCAG, CVD simulation, CSS generators |
| `dataviz` | cdylib | data_visualization | SVG charts, histograms, pivot, profiling, outliers, sampling |
| `document` | cdylib | document, ebook | DOCX/PPTX/ODT/RTF read, DOCX write, template fill, Markdown→DOCX, EPUB/MOBI |
| `font` | cdylib | font | ttf-parser inspect/char map, ab_glyph PNG preview, WOFF1↔TTF/OTF, subsetting with rebuilt cmap |
| `mesh3d` | cdylib | 3d | STL/OBJ parse+convert, inspect/volume/watertight, transforms, basic repair, GLB↔glTF, thumbnails |
| `search` | cdylib | – | catalog fuzzy search (own protocol) |

## Build, test, lint

```bash
rustup target add wasm32-unknown-unknown
pnpm run fmt:engines          # scripts/format-extra.py + cargo fmt (see below)
pnpm run lint:engines         # clippy -D warnings, cargo fmt --check, format-extra.py --check
pnpm run test:engines         # cargo test --workspace: unit + protocol tests (native)
pnpm run build:engines        # build all, copy to frontend/public/engines, write shared/data/engines.json
pnpm run test:engines:wasm    # instantiate every .wasm like the worker does; hash/size/actions/valid/invalid/empty
pnpm run test:engines:ui      # generated forms → payload → real .wasm for every action; SSR of the React runner
pnpm run check:engines        # all of the above, in CI order
pnpm run generate:engine-tools   # regenerate engine-schemas.json (from Rust) and engine-tools.json (from catalog.json)
```

`engines/.cargo/config.toml` sets `--cfg getrandom_backend="custom"` for the
wasm target so `getrandom` routes through the shared entropy pool.

### Code quality rules (enforced)

Configured once in `engines/Cargo.toml` (`[workspace.lints]`) and inherited by
every crate through `[lints] workspace = true`:

| rule | why |
|---|---|
| `clippy::unwrap_used`, `expect_used`, `panic`, `unreachable`, `todo`, `unimplemented` = **deny** | the wasm build is `panic = "abort"`; a panic kills the engine instance. Tests are exempt (`engines/clippy.toml`). |
| `clippy::dbg_macro`, `print_stdout`, `print_stderr` = deny | there is no stdout in the worker |
| `rustc::missing_docs` = warn (CI: `-D warnings`) | every public item, action variant and payload field is documented – the enums are the API reference |
| `rustc::unsafe_op_in_unsafe_fn` = warn | every `unsafe` block carries a `// SAFETY:` comment |
| `cargo fmt --check` with `engines/rustfmt.toml` | stable-only options, 100 columns |
| `scripts/format-extra.py --check` | reflows `json!` bodies, splits long string literals with `\` continuations and wraps long doc comments – the three things stable rustfmt cannot do; only whitespace changes and the tool verifies that |

Helpers that make the no-panic rule easy: `EngineError::*` constructors,
`engine_shared::util::cmp_partial` / `f64::total_cmp` for float sorting,
`engine_shared::rng::pick` for random choice, crate-local `re()` helpers for
built-in regular expressions. See `CONTRIBUTING.md` for the full guide.

### Testing requirements per engine

Every engine has `tests/engine_api.rs` exercising the JSON protocol with
**valid**, **invalid** and **empty** inputs plus unit tests for the core
algorithms; `scripts/test-engines.mjs` repeats the three classes against the
compiled `.wasm` (wasm compilation test) and verifies that every action named
in `shared/data/engine-tools.json` exists.

## Generic tool UI (no JSON, no base64)

`shared/data/engine-schemas.json` is generated from the Rust action enums by
`scripts/generate-engine-schemas.py`: every variant becomes a form with one
widget per payload field – file pickers (drag & drop, previews, ordering) for
`Vec<u8>` fields, numbers with bounds/sliders, switches, selects built from the
doc comment option lists (`` `a` (default), `b` or `c` ``), colour pickers,
tag lists, matrices, key/value editors, repeatable groups for `Vec<Struct>`
and variant pickers for internally tagged enums. The frontend
(`frontend/src/tools/engine/`) renders the form, base64-encodes uploads
behind the scenes, injects `entropy` / `now`, and renders results as download
cards with previews, SVG/HTML previews, tables and stat cards.

Because the schema is derived from the source, **the doc comment of a payload
field is UI copy**: list the accepted values in back-ticks, mark the default,
and give ranges as `1–100`. Tool-specific presets (e.g. `png-to-webp` →
`format: webp`) live in `scripts/generate-engine-tools.py` (`PRESETS`,
`REGISTRY_ALIASES`). `pnpm run test:engines:ui` fills every action's form with
sample data, sends the generated payload to the real `.wasm` and fails when an
engine rejects the payload shape.

## Adding a tool

1. Implement the action in the right engine (`handle_action` match arm + enum
   variant, `#[serde(rename_all = "snake_case")]`). Validate inputs with
   `engine_shared::error::require_*` / `limits::*` and return typed errors.
2. Add unit tests and a protocol case in `tests/engine_api.rs`.
3. Map the catalog tool in `scripts/generate-engine-tools.py` (`MAP`, optional
   `PRESETS` / `REGISTRY_ALIASES`) and run `pnpm run generate:engine-tools`.
4. `node scripts/build-engines.mjs` → updates binaries and `engines.json`.
5. The frontend exposes the tool automatically through `EngineToolRunner`
   (`frontend/src/tools/engine/EngineToolRunner.tsx`); add a dedicated UI
   component later and register it in `frontend/src/tools/index.ts` +
   `DEDICATED_UI` in `tool-registry.ts`.

## Implementation status (catalog.json, 764 tools)

| status | tools | meaning |
|---|---|---|
| implemented | 553 | WASM action with tests |
| partial | 47 | works with documented limits (e.g. lossless-only video trim, token-level JS formatter) |
| unsupported | 24 | action returns `E_UNSUPPORTED` naming the correct runtime (codecs, TS compiler, 7z/rar, HEIC) |
| browser | 109 | catalog runtime is JavaScript (seo, network, productivity, …) – no WASM work required |
| backend | 8 | seo_crawler – needs network |
| not-started | 23 | ocr, ai_local, speech (ML models – see below) |

### Not started / design notes

* **ocr, ai_local, speech** – need model weights (Tesseract/ONNX/Whisper);
  plan: WebGPU/WASM inference via `tract`/`candle` with separately
  distributed, lazily loaded model packs. Not feasible without model assets.
* **WOFF2** (`font-converter`) – needs Brotli plus the glyf/loca transform;
  `font.convert` returns `E_UNSUPPORTED` pointing at the browser `FontFace`
  API / backend `woff2_decompress`.
* **glTF geometry / point clouds** – `mesh3d` handles STL/OBJ geometry and
  glTF *containers*; decoding glTF accessors into triangles, Draco/meshopt
  compression and PLY/PCD point clouds are left to the browser WebGL viewer.
* **code_analysis sandboxes** – cloc-style counting, complexity, duplicates
  and import sorting live in `developer-engine` (`code.rs`); playgrounds and
  sandboxes belong to the browser JavaScript runtime (`E_UNSUPPORTED`).

Each `engine-tools.json` entry records `engine`, `action`, `status`, `note`,
`requires` (`files` / `text`) and `network` (always `false` for WASM engines).
