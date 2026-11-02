# Dev Toolbox

A free directory of everyday developer and creator utilities, with 710 searchable tool listings. Tools are currently placeholders, not working utilities.

## Run & Operate

- `pnpm --filter @workspace/api-server run dev` — run the API server (port 5000)
- `pnpm --filter @workspace/dev-toolbox run dev` — run the website
- `pnpm --filter @workspace/dev-toolbox run build:wasm` — rebuild the browser search binary from Rust
- `pnpm run typecheck` — full typecheck across all packages
- `pnpm run build` — typecheck + build all packages
- `pnpm --filter @workspace/api-spec run codegen` — regenerate API hooks and Zod schemas from the OpenAPI spec
- `pnpm --filter @workspace/db run push` — push DB schema changes (dev only)
- The directory is client-only and does not require a database or API.

## Stack

- pnpm workspaces, Node.js 24, TypeScript 5.9
- Frontend: React, Vite, Wouter
- Browser search: Rust compiled to WebAssembly; precompiled binary ships with the website
- API: Express 5
- DB: PostgreSQL + Drizzle ORM
- Validation: Zod (`zod/v4`), `drizzle-zod`
- API codegen: Orval (from OpenAPI spec)
- Build: esbuild (CJS bundle)

## Where things live

_Populate as you build — short repo map plus pointers to the source-of-truth file for DB schema, API contracts, theme files, etc._

## Architecture decisions

- Utility implementations are outside the current scope; all listings must remain clearly marked as coming soon.
- Search supports exact, prefix, multiword, and typo-tolerant matching in the browser.
- Tool pages are statically generated but marked noindex until they contain working tools and substantive content, avoiding thin-content indexing.

## Product

Users can browse the full catalog, filter by category, search by name, and open a dedicated placeholder page for each tool. The site is free and is primarily intended for developers.

## User preferences

_Populate as you build — explicit user instructions worth remembering across sessions._

## Gotchas

- Set `artifacts/dev-toolbox/seo.config.json` to the verified published URL after publishing, then rebuild to generate the canonical URLs and sitemap. Do not use the development preview URL.
- The precompiled WASM binary is committed so ordinary website builds do not need to download a Rust toolchain.

## Pointers

- See the `pnpm-workspace` skill for workspace structure, TypeScript setup, and package details
