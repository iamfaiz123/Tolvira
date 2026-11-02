# Contributing to Dev Toolbox

Thanks for helping! This document covers the whole repository; the WASM
engines have extra rules because a single panic there takes a tool down for
the user, so please read [Engine rules](#engine-rules) carefully before
touching `engines/`.

- [Repository layout](#repository-layout)
- [Getting started](#getting-started)
- [Workflow](#workflow)
- [Engine rules](#engine-rules)
  - [Never panic](#never-panic)
  - [Errors](#errors)
  - [Documentation](#documentation)
  - [Formatting](#formatting)
  - [Linting](#linting)
  - [Tests](#tests)
  - [Adding an action](#adding-an-action)
  - [Adding an engine](#adding-an-engine)
- [Frontend and backend](#frontend-and-backend)
- [Commit messages and pull requests](#commit-messages-and-pull-requests)
- [Licensing](#licensing)

## Repository layout

| path | what |
|---|---|
| `catalog.json` | the tool catalog: every tool, its category and the engine that serves it |
| `engines/` | Rust workspace compiled to `wasm32-unknown-unknown` (one crate per engine, see `engines/README.md`) |
| `frontend/` | React/Vite app; loads engines in a Web Worker (`src/workers/engine.worker.ts`) |
| `backend/` | Rust service for tools that need network access; shares `shared/data/*.json` |
| `shared/data/engines.json` | engine manifest (id, wasm path, sha256, capabilities) – **generated** by `scripts/build-engines.mjs` |
| `shared/data/engine-tools.json` | catalog tool → engine/action map and status – **generated** by `scripts/generate-engine-tools.py` |
| `scripts/` | build, test and formatting helpers (see below) |

## Getting started

```bash
# toolchain
rustup toolchain install stable --component clippy rustfmt
rustup target add wasm32-unknown-unknown
corepack enable && pnpm install          # Node 20+

# everything the CI runs
pnpm run check:engines                   # fmt + clippy + tests + wasm build + wasm smoke tests
pnpm run dev:frontend                    # http://localhost:5173
```

Useful individual commands:

| command | purpose |
|---|---|
| `pnpm run fmt:engines` | format the engines (`format-extra.py` + `cargo fmt`) |
| `pnpm run lint:engines` | clippy with `-D warnings`, `cargo fmt --check`, `format-extra.py --check` |
| `pnpm run test:engines` | `cargo test --workspace` (unit + protocol tests, native) |
| `pnpm run build:engines` | build every engine to wasm, copy to `frontend/public/engines/`, write `engines.json` |
| `pnpm run test:engines:wasm` | instantiate every `.wasm` like the worker does and run valid/invalid/empty cases |
| `pnpm run generate:engine-tools` | regenerate `shared/data/engine-schemas.json` (forms, from the Rust enums) and `engine-tools.json` (from `catalog.json`) |
| `pnpm run test:engines:ui` | fill every action's generated form with sample data, run the payload through the real `.wasm`, SSR-render the React runner |

## Workflow

1. Open an issue (or pick one) describing the tool/bug. For new tools check
   `catalog.json` first – the tool id and engine assignment live there.
2. Branch from `main`: `feat/<engine>-<tool>`, `fix/<engine>-<what>`, `docs/…`.
3. Implement with tests (see [Tests](#tests)).
4. Run `pnpm run check:engines` (and `pnpm run typecheck` if you touched TS).
5. Open a pull request using the checklist at the bottom of this file.

## Engine rules

Engines are `cdylib` crates in `engines/`. They speak one JSON protocol
(`{action, payload}` → `{success, data}` / `{success:false, error, error_code}`)
documented in `engines/README.md`, and have **no imports**: no clock, no
randomness, no network, no files. Whatever the tool needs is passed in the
payload (`now`, `entropy`, base64 files…).

### Never panic

The wasm build uses `panic = "abort"`; a panic is an `unreachable` trap that
kills the whole engine instance and every tool served by it. Therefore, in
non-test code:

* **No `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!`,
  `unimplemented!`.** These are hard clippy errors for the whole workspace
  (`[workspace.lints]` in `engines/Cargo.toml`; tests are exempt through
  `engines/clippy.toml`).
* Prefer `?` with a typed `EngineError`; fall back to `unwrap_or(..)` /
  `unwrap_or_default()` / `let … else { return Err(..) }` only when a sensible
  default exists. Add a one-line comment when the fallback is "cannot happen"
  so the next reader knows the reasoning.
* Float ordering: `a.total_cmp(&b)` or `engine_shared::util::cmp_partial`,
  never `partial_cmp(..).unwrap()`.
* Random picks: `engine_shared::rng::pick(items, "what")` returns an
  `E_EMPTY_INPUT` instead of panicking on an empty slice.
* Regular expressions that are part of the source: compile through the
  crate-local `re(pattern)` helper (text/developer engines) which maps a
  failure to `E_INTERNAL`.
* Avoid direct indexing on untrusted data (`bytes[80]`, `parts[1]`). Use
  `.get(..)`, `split_first()`, `chunks_exact()` or validate the length first
  and say so in a comment. `clippy::indexing_slicing` is not (yet) enforced,
  but reviewers will ask.
* Guard size and count with `engine_shared::limits` (`E_LIMIT_EXCEEDED`)
  before allocating proportional to input.
* `unsafe` is only allowed in `engine-shared` (allocator/RNG glue) and the
  `search` crate's ABI; every block needs a `// SAFETY:` comment.

### Errors

Use the constructors on `EngineError` – the code is part of the public
contract (the frontend maps it to UI states):

| constructor | code | when |
|---|---|---|
| `bad_request` | `E_BAD_REQUEST` | request envelope / payload shape wrong (serde does this for you) |
| `empty` | `E_EMPTY_INPUT` | required text / file / list missing |
| `invalid` | `E_INVALID_INPUT` | present but wrong: ranges, unknown option names, bad references |
| `unsupported_format` | `E_UNSUPPORTED_FORMAT` | bytes are not a format this action handles |
| `malformed` | `E_MALFORMED` | format recognised but corrupt / truncated |
| `encrypted` / `wrong_password` | `E_ENCRYPTED` / `E_WRONG_PASSWORD` | protected documents |
| `limit` | `E_LIMIT_EXCEEDED` | size / count / time guard |
| `not_found` | `E_NOT_FOUND` | page, sheet, column, key… does not exist |
| `unsupported` | `E_UNSUPPORTED` | deliberately not implemented in WASM – the message **must** name the runtime that can do it |
| `entropy_required` | `E_ENTROPY_REQUIRED` | caller must send `payload.entropy` |
| `internal` | `E_INTERNAL` | a bug; only for "cannot happen" branches |

Messages are user-facing: say what was wrong and what to do
(``"`quality` must be between 1 and 100"``), never dump debug output.

### Documentation

`missing_docs` is a warning for the whole workspace and CI runs clippy with
`-D warnings`, so **every public item needs a doc comment**:

* Every crate starts with a `//!` header: what the engine does, its action
  list (or a table mapping actions to catalog tools) and its limits.
* Action enums: document the variant (what the tool does) **and every payload
  field** (meaning, unit, default). These comments are the API reference the
  frontend and backend teams read **and they drive the generated UI**: values
  listed in back-ticks (`` `scale` (default), `center` or `stretch` ``) become a
  select, a range written as `1–100` becomes input bounds / a slider, and
  `(base64)` is stripped from file fields.
* Functions: one sentence on what they do, then inputs / units / error cases
  if not obvious. Mention `E_…` codes that callers may want to handle.
* Use back-ticks for code, `E_CODES`, option values (`` `scale` (default) ``)
  and units (`dBFS`, `px`).
* Keep doc lines ≤ 100 columns; `scripts/format-extra.py` re-wraps long ones
  (tables and code fences are left alone).

### Formatting

Formatting is fully automated – run `pnpm run fmt:engines` before committing.
It executes:

1. `python3 scripts/format-extra.py` – the three things stable rustfmt cannot
   do: reflow `json!({ … })` bodies one key per line, split long string
   literals with `\` continuations (value-preserving), wrap long doc comments;
2. `cargo fmt --all` with `engines/rustfmt.toml` (stable options only:
   100 columns, 4 spaces, reordered imports/modules, trailing commas in match
   arms, field-init shorthand, `?` shorthand);
3. `format-extra.py` + `cargo fmt` once more so continuation strings are
   aligned to their final column.

`format-extra.py` verifies that only whitespace changed (decoded string values
and the code token stream are compared) and refuses to write otherwise.
Nightly users can additionally use `engines/rustfmt.nightly.toml`
(`cargo +nightly fmt --all -- --config-path engines/rustfmt.nightly.toml`);
the result must still pass the stable check.

Style notes the formatter cannot enforce:

* One `json!` entry per line for anything longer than a short object; nested
  objects get their own block.
* Prefer `let Some(x) = … else { … }` over nested `match` for early returns.
* Keep functions under ~80 lines; split big actions into helpers in a module.

### Linting

`pnpm run lint:engines` must be clean:

```bash
cargo clippy --manifest-path engines/Cargo.toml --workspace --all-targets -- -D warnings
cargo fmt --manifest-path engines/Cargo.toml --all -- --check
python3 scripts/format-extra.py --check
```

Lint configuration lives in one place – `engines/Cargo.toml`
`[workspace.lints]` – and every crate opts in with `[lints] workspace = true`.
Do not add crate-level `#![allow(..)]`; if a lint is wrong for a specific
expression, allow it on that item with a comment explaining why.

### Tests

Every engine has two layers of tests and CI runs both natively and against
the compiled wasm:

1. **Unit tests** (`#[cfg(test)] mod tests` next to the code) for the core
   algorithms – parsing, maths, encoders. Use real fixtures from
   `engines/<crate>/tests/fixtures/` (small: a few KB) rather than synthetic
   happy paths.
2. **Protocol tests** (`engines/<crate>/tests/engine_api.rs`) that call
   `process_request` with JSON exactly like the worker does. Each action needs
   at least:
   * a **valid** case asserting the important output fields,
   * an **invalid** case (wrong type, out of range, corrupt file) asserting
     the `error_code`,
   * an **empty** case (`{}` and empty strings/files) asserting
     `E_EMPTY_INPUT`.
3. **WASM smoke tests** – add a `CASES` entry in `scripts/test-engines.mjs`
   for every new engine (and for new actions where the fixture is cheap). The
   script instantiates the real `.wasm`, checks hash/size against
   `engines.json`, verifies every action referenced by `engine-tools.json`
   exists, and runs the cases.

For file formats, test the unhappy paths explicitly: truncated files,
encrypted documents with and without the right password, wrong magic bytes,
zero-length inputs, and the size limits. Test code may use `unwrap()`.

### Adding an action

1. Add a variant to the engine's action enum
   (`#[serde(rename_all = "snake_case")]`), document it and its fields, and
   give optional fields `#[serde(default)]` with sensible defaults.
2. Implement it in the right module; validate inputs first
   (`engine_shared::error::require_*`, `limits::*`), return typed errors.
3. Add unit + protocol tests (valid / invalid / empty).
4. Map the catalog tool in `scripts/generate-engine-tools.py` (`MAP`) with
   status `I` (implemented), `P` (partial – say what is missing) or `U`
   (unsupported – the action must return `E_UNSUPPORTED` with guidance). Add a
   `PRESETS` entry when the tool name implies a parameter (`png-to-webp` →
   `format: webp`) and a `REGISTRY_ALIASES` entry for registry slugs served by
   the same action; run `pnpm run generate:engine-tools` (schemas + tools).
5. `pnpm run build:engines && pnpm run test:engines:wasm`.
6. Update the action table in the crate's `//!` header and, if the tool
   count changed, the status table in `engines/README.md`.

### Adding an engine

1. `cargo new --lib engines/<name>`; copy `Cargo.toml` from a small engine
   (`cdylib` + `rlib`, `[lints] workspace = true`, `engine-shared` dep) and add
   the crate to `engines/Cargo.toml` members.
2. Define the action enum, `handle_action`, and finish `lib.rs` with
   `engine_shared::export_engine!(handle_action, YourAction);` – this emits
   the wasm ABI and `process_request` for tests.
3. Only pure-Rust, `wasm32-unknown-unknown`-capable dependencies (no
   `std::fs`, no threads, no C). If a crate needs randomness, route it through
   `engine-shared`'s custom `getrandom` backend (already configured in
   `engines/.cargo/config.toml`).
4. Register it in `scripts/build-engines.mjs` (`ENGINES`), map it in
   `scripts/generate-engine-tools.py` (`ENGINES` + `MAP`, remove it from
   `PLANNED`), add smoke cases to `scripts/test-engines.mjs`, and document it
   in `engines/README.md` (layout and status tables).
5. Keep the binary small: `opt-level = "z"`, no `format!`-heavy hot paths, no
   large embedded assets unless the tool needs them (fonts/rules are the
   exception and are licensed in the crate's `assets/` folder).

## Frontend and backend

* TypeScript is formatted with Prettier and type-checked with
  `pnpm run typecheck`.
* The frontend never special-cases an engine: `engine-registry.ts` reads
  `engines.json`, `tool-registry.ts` reads `engine-tools.json`,
  `engine-schemas.ts` reads `engine-schemas.json`, and `EngineToolRunner`
  renders any tool from its schema (file pickers, numbers, selects, keypad
  for expressions, result cards/previews/tables). Users never see JSON or
  base64. Add a dedicated UI only when the generic runner is insufficient
  (interactive canvas, live preview) and register it in `DEDICATED_UI`.
* New field widgets go into `frontend/src/tools/engine/fields.tsx` and must be
  covered by `frontend/tests/engine-ui.ts` (`sampleValue` + `buildPayload`).
* Pass `error_code` through untouched; the UI keys messages on it.
* Backend tools (network crawlers etc.) live in `backend/`; they must also
  use the shared tool ids from `catalog.json`.

## Commit messages and pull requests

* Conventional commits: `feat(pdf): add redact action`, `fix(audio): reject
  empty FLAC`, `docs(engines): document error codes`, `chore(lints): …`.
* One engine or topic per PR where possible; regenerate the JSON data files in
  the same PR as the change that needs them.
* Rebuilt `.wasm` binaries are committed together with their `engines.json`
  hashes (`pnpm run build:engines`) so that `test:engines:wasm` matches.

PR checklist:

- [ ] `pnpm run check:engines` passes (fmt, clippy `-D warnings`, tests, wasm build, wasm smoke tests)
- [ ] no `unwrap`/`expect`/`panic` outside tests; new `unsafe` has a `SAFETY:` comment
- [ ] every new public item and payload field is documented
- [ ] valid / invalid / empty tests for every new action
- [ ] `engine-tools.json` / `engines.json` regenerated, `engines/README.md` updated
- [ ] user-facing error messages say what to do; `E_UNSUPPORTED` names the right runtime

## Licensing

By contributing you agree that your contributions are licensed under the
repository's MIT license. Third-party assets (fonts, rule tables) must be
redistributable and carry their license file next to them.
