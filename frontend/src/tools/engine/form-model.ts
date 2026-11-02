import type { FieldOption, FieldSchema, ListItemSchema } from "../../registry/engine-schemas";

/**
 * Form model shared by the generic engine UI: initial values, validation and
 * conversion of the form state into the JSON payload an engine expects.
 *
 * Form values mirror the schema: scalars are strings/numbers/booleans,
 * `file` fields hold a `File`, `files` hold `File[]`, groups are objects,
 * lists are arrays, `keyvalues` are `{ key, value }[]` and `variant` values
 * are `{ [tag]: string, ...fields }`.
 */
export type FormValue = any;
export type FormValues = Record<string, FormValue>;

export interface PayloadProblem {
  path: string;
  message: string;
}

/** Fields the user normally does not see: filled automatically by the UI. */
export function isAutoField(f: FieldSchema): boolean {
  return Boolean(f.auto);
}

/** `name` next to a `file` field is filled from the uploaded file name. */
export function isDerivedName(f: FieldSchema, siblings: FieldSchema[]): boolean {
  return f.name === "name" && f.kind === "text" && siblings.some((s) => s.kind === "file" || s.kind === "files");
}

/** Builds the default value of a single field. */
export function defaultValue(f: FieldSchema): FormValue {
  switch (f.kind) {
    case "boolean":
      return typeof f.default === "boolean" ? f.default : false;
    case "number":
    case "integer":
      if (f.name === "n" && (f.default === 0 || f.default === undefined)) return 1;
      return typeof f.default === "number" ? f.default : "";
    case "text":
    case "textarea":
    case "color":
      return typeof f.default === "string" ? f.default : "";
    case "select":
      return typeof f.default === "string" ? f.default : f.required ? (f.options?.[0]?.value ?? "") : "";
    case "file":
      return null;
    case "files":
      return [];
    case "tags":
      return Array.isArray(f.default) ? f.default.join(", ") : "";
    case "numbers":
      return Array.isArray(f.default) ? f.default.join(", ") : "";
    case "matrix":
      return Array.isArray(f.default) ? (f.default as unknown[][]).map((r) => (r as unknown[]).join(", ")).join("\n") : "";
    case "json":
      return f.default !== undefined ? JSON.stringify(f.default, null, 2) : "";
    case "keyvalues":
      return [];
    case "group":
      return initialValues(f.fields ?? []);
    case "list":
      return [];
    case "variant":
      return variantDefault(f.tag ?? "op", f.variants ?? [], f.variants?.[0]?.value);
    default:
      return "";
  }
}

export function variantDefault(tag: string, variants: FieldOption[], value?: string): FormValues {
  const v = variants.find((o) => o.value === value) ?? variants[0];
  const out: FormValues = { [tag]: v?.value ?? "" };
  Object.assign(out, initialValues(v?.fields ?? []));
  return out;
}

export function listItemDefault(item: ListItemSchema): FormValue {
  if (item.kind === "variant") return variantDefault(item.tag ?? "op", item.variants ?? []);
  return initialValues(item.fields ?? []);
}

/** Initial values for a field list, optionally overlaid with tool presets. */
export function initialValues(fields: FieldSchema[], preset?: Record<string, unknown>): FormValues {
  const out: FormValues = {};
  for (const f of fields) {
    out[f.name] = defaultValue(f);
    if (preset && preset[f.name] !== undefined) out[f.name] = presetToForm(f, preset[f.name]);
  }
  return out;
}

/** Converts a preset (JSON payload value) into the form representation. */
export function presetToForm(f: FieldSchema, value: unknown): FormValue {
  switch (f.kind) {
    case "tags":
    case "numbers":
      return Array.isArray(value) ? value.join(", ") : String(value ?? "");
    case "matrix":
      return Array.isArray(value) ? (value as unknown[][]).map((r) => r.join(", ")).join("\n") : "";
    case "json":
      return typeof value === "string" ? value : JSON.stringify(value, null, 2);
    case "keyvalues":
      return value && typeof value === "object" ? Object.entries(value as Record<string, unknown>).map(([key, v]) => ({ key, value: typeof v === "string" ? v : JSON.stringify(v) })) : [];
    case "group": {
      const inner = initialValues(f.fields ?? []);
      if (value && typeof value === "object") {
        for (const sub of f.fields ?? []) {
          const v = (value as Record<string, unknown>)[sub.name];
          if (v !== undefined) inner[sub.name] = presetToForm(sub, v);
        }
      }
      return inner;
    }
    case "list": {
      if (!Array.isArray(value) || !f.item) return [];
      return value.map((entry) => {
        if (f.item!.kind === "variant") {
          const tag = f.item!.tag ?? "op";
          const base = variantDefault(tag, f.item!.variants ?? [], (entry as Record<string, string>)[tag]);
          const variant = f.item!.variants?.find((o) => o.value === (entry as Record<string, string>)[tag]);
          for (const sub of variant?.fields ?? []) {
            const v = (entry as Record<string, unknown>)[sub.name];
            if (v !== undefined) base[sub.name] = presetToForm(sub, v);
          }
          return base;
        }
        const base = initialValues(f.item!.fields ?? []);
        for (const sub of f.item!.fields ?? []) {
          const v = (entry as Record<string, unknown>)[sub.name];
          if (v !== undefined) base[sub.name] = presetToForm(sub, v);
        }
        return base;
      });
    }
    case "variant": {
      const tag = f.tag ?? "op";
      const base = variantDefault(tag, f.variants ?? [], (value as Record<string, string>)?.[tag]);
      const variant = f.variants?.find((o) => o.value === (value as Record<string, string>)?.[tag]);
      for (const sub of variant?.fields ?? []) {
        const v = (value as Record<string, unknown>)?.[sub.name];
        if (v !== undefined) base[sub.name] = presetToForm(sub, v);
      }
      return base;
    }
    case "boolean":
      return Boolean(value);
    case "number":
    case "integer":
      return typeof value === "number" ? value : value === "" || value == null ? "" : Number(value);
    default:
      return value == null ? "" : String(value);
  }
}

// ------------------------------------------------------------------ helpers

export function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).split(",")[1] ?? "");
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

/** Secure random bytes for engines that need caller-provided entropy. */
export function entropy(): string {
  const buf = new Uint8Array(32);
  crypto.getRandomValues(buf);
  let s = "";
  buf.forEach((b) => (s += String.fromCharCode(b)));
  return btoa(s);
}

export function autoValue(kind: string): unknown {
  const now = new Date();
  switch (kind) {
    case "entropy":
      return entropy();
    case "now_iso":
      return now.toISOString();
    case "now_unix":
      return Math.floor(now.getTime() / 1000);
    case "now_ms":
      return now.getTime();
    case "today_serial":
      return Math.floor((now.getTime() - Date.UTC(1899, 11, 30)) / 86_400_000);
    default:
      return undefined;
  }
}

function splitList(text: string): string[] {
  return text
    .split(/[\n,]/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

function parseNumberList(text: string, path: string, problems: PayloadProblem[]): number[] {
  const out: number[] = [];
  for (const part of text.split(/[\s,;]+/).filter(Boolean)) {
    const n = Number(part);
    if (!Number.isFinite(n)) problems.push({ path, message: `"${part}" is not a number` });
    else out.push(n);
  }
  return out;
}

function isEmpty(v: unknown): boolean {
  if (v === undefined || v === null) return true;
  if (typeof v === "string") return v.trim() === "";
  if (Array.isArray(v)) return v.length === 0;
  if (typeof v === "object") return Object.keys(v as object).length === 0;
  return false;
}

// ---------------------------------------------------------- mode dependencies

export interface FieldDisabledState {
  disabled: boolean;
  reason?: string;
}

/** Determines if a field is disabled based on the active mode or strategy. */
export function isFieldDisabled(
  field: FieldSchema,
  values: FormValues,
  siblings: FieldSchema[]
): FieldDisabledState {
  const modeVal = values.mode ?? values.strategy ?? values.type;

  // Tools with a mode field (such as split-pdf)
  if (siblings.some((s) => s.name === "mode")) {
    const currentMode = String(modeVal ?? "each");

    if (field.name === "ranges" || field.name === "range") {
      if (currentMode !== "ranges") {
        return {
          disabled: true,
          reason: 'Only for "Ranges" mode',
        };
      }
    }

    if (field.name === "n" || field.name === "chunk_size" || field.name === "every_n") {
      if (currentMode !== "every_n") {
        return {
          disabled: true,
          reason: 'Only for "Every n" mode',
        };
      }
    }
  }

  // Generic doc-based mode checking (e.g. "used by [`...::Ranges`]" or "used in `ranges` mode")
  if (field.doc && modeVal !== undefined) {
    const currentMode = String(modeVal).toLowerCase().replace(/_/g, "");
    const doc = field.doc;

    const usedByMatch =
      doc.match(/used by \[`?[A-Za-z0-9_:]+::([A-Za-z0-9_]+)`?\]/i) ||
      doc.match(/used in [`"']?([A-Za-z0-9_]+)[`"']? mode/i);

    if (usedByMatch) {
      const targetMode = usedByMatch[1].toLowerCase().replace(/_/g, "");
      if (currentMode !== targetMode) {
        return {
          disabled: true,
          reason: `Only for "${usedByMatch[1]}" mode`,
        };
      }
    }
  }

  return { disabled: false };
}

// ---------------------------------------------------------- payload builder

/**
 * Converts form values into the JSON payload, reporting validation problems.
 * Files are base64-encoded here so the rest of the UI never sees base64.
 */
export async function buildPayload(fields: FieldSchema[], values: FormValues, path = ""): Promise<{ payload: Record<string, unknown>; problems: PayloadProblem[] }> {
  const payload: Record<string, unknown> = {};
  const problems: PayloadProblem[] = [];
  const loc = (f: FieldSchema) => (path ? `${path}.${f.label}` : f.label);

  for (const f of fields) {
    const disabledState = isFieldDisabled(f, values, fields);
    if (disabledState.disabled) {
      // Do not validate or send inputs that are inactive for the selected mode
      continue;
    }
    const v = values[f.name];
    if (f.auto) {
      const auto = isEmpty(v) ? autoValue(f.auto) : v;
      if (auto !== undefined) payload[f.name] = auto;
      continue;
    }
    switch (f.kind) {
      case "file": {
        const file = v as File | null;
        if (!file) {
          if (f.required) problems.push({ path: loc(f), message: "Please choose a file" });
          break;
        }
        payload[f.name] = await fileToBase64(file);
        if (fields.some((s) => isDerivedName(s, fields)) && isEmpty(values.name)) payload.name = file.name;
        break;
      }
      case "files": {
        const list = (v as File[]) ?? [];
        if (list.length === 0) {
          if (f.required) problems.push({ path: loc(f), message: "Please choose at least one file" });
          break;
        }
        payload[f.name] = await Promise.all(list.map(fileToBase64));
        break;
      }
      case "boolean":
        payload[f.name] = Boolean(v);
        break;
      case "number":
      case "integer": {
        if (isEmpty(v)) {
          if (f.required || (f.name === "n" && values.mode === "every_n")) {
            problems.push({ path: loc(f), message: "Required" });
          }
          break;
        }
        const n = typeof v === "number" ? v : Number(String(v).replace(/,/g, "."));
        if (!Number.isFinite(n)) {
          problems.push({ path: loc(f), message: `"${v}" is not a number` });
          break;
        }
        if (f.name === "n" && values.mode === "every_n" && n < 1) {
          problems.push({ path: loc(f), message: "Must be at least 1 page per split file" });
          break;
        }
        if (f.min !== undefined && n < f.min) problems.push({ path: loc(f), message: `Must be at least ${f.min}` });
        if (f.max !== undefined && n > f.max) problems.push({ path: loc(f), message: `Must be at most ${f.max}` });
        payload[f.name] = f.kind === "integer" ? Math.round(n) : n;
        break;
      }
      case "text":
      case "textarea":
      case "select":
      case "color": {
        if (isEmpty(v)) {
          if (f.required) problems.push({ path: loc(f), message: "Required" });
          else if (f.name === "name" && isDerivedName(f, fields)) break;
          break;
        }
        payload[f.name] = String(v);
        break;
      }
      case "tags": {
        const items = splitList(String(v ?? ""));
        if (items.length === 0) {
          if (f.required || (f.name === "ranges" && values.mode === "ranges")) {
            problems.push({ path: loc(f), message: "Please specify at least one page range (e.g. 1-3, 4-6)" });
          }
          break;
        }
        payload[f.name] = items;
        break;
      }
      case "numbers": {
        if (isEmpty(v)) {
          if (f.required) problems.push({ path: loc(f), message: "Enter at least one number" });
          break;
        }
        const nums = parseNumberList(String(v), loc(f), problems);
        if (f.fixed && nums.length !== f.fixed) problems.push({ path: loc(f), message: `Exactly ${f.fixed} numbers are needed` });
        payload[f.name] = nums;
        break;
      }
      case "matrix": {
        if (isEmpty(v)) {
          if (f.required) problems.push({ path: loc(f), message: "Enter at least one row" });
          break;
        }
        payload[f.name] = String(v)
          .split("\n")
          .map((r) => r.trim())
          .filter(Boolean)
          .map((r) => parseNumberList(r, loc(f), problems));
        break;
      }
      case "json": {
        if (isEmpty(v)) {
          if (f.required) problems.push({ path: loc(f), message: "Required" });
          break;
        }
        try {
          payload[f.name] = JSON.parse(String(v));
        } catch (e) {
          problems.push({ path: loc(f), message: `Invalid JSON: ${(e as Error).message}` });
        }
        break;
      }
      case "keyvalues": {
        const rows = (v as { key: string; value: string }[]) ?? [];
        const obj: Record<string, unknown> = {};
        for (const row of rows) {
          if (!row.key.trim()) continue;
          let val: unknown = row.value;
          if (f.valueKind === "number") {
            const n = Number(row.value);
            if (!Number.isFinite(n)) {
              problems.push({ path: `${loc(f)}.${row.key}`, message: `"${row.value}" is not a number` });
              continue;
            }
            val = n;
          } else if (f.valueKind === "any") {
            try {
              val = JSON.parse(row.value);
            } catch {
              val = row.value;
            }
          }
          obj[row.key.trim()] = val;
        }
        if (Object.keys(obj).length === 0) {
          if (f.required) problems.push({ path: loc(f), message: "Add at least one entry" });
          break;
        }
        payload[f.name] = obj;
        break;
      }
      case "group": {
        const inner = await buildPayload(f.fields ?? [], (v as FormValues) ?? {}, loc(f));
        problems.push(...inner.problems);
        if (Object.keys(inner.payload).length > 0 || f.required) payload[f.name] = inner.payload;
        break;
      }
      case "list": {
        const items = (v as FormValues[]) ?? [];
        if (items.length === 0) {
          if (f.required) problems.push({ path: loc(f), message: "Add at least one item" });
          break;
        }
        const out: unknown[] = [];
        for (let i = 0; i < items.length; i++) {
          out.push(await buildItem(f.item!, items[i], `${loc(f)}[${i + 1}]`, problems));
        }
        payload[f.name] = out;
        break;
      }
      case "variant": {
        payload[f.name] = await buildVariant(f.tag ?? "op", f.variants ?? [], v as FormValues, loc(f), problems);
        break;
      }
      default:
        if (!isEmpty(v)) payload[f.name] = v;
    }
  }
  return { payload, problems };
}

async function buildItem(item: ListItemSchema, value: FormValues, path: string, problems: PayloadProblem[]): Promise<unknown> {
  if (item.kind === "variant") return buildVariant(item.tag ?? "op", item.variants ?? [], value, path, problems);
  const inner = await buildPayload(item.fields ?? [], value ?? {}, path);
  problems.push(...inner.problems);
  return inner.payload;
}

async function buildVariant(tag: string, variants: FieldOption[], value: FormValues, path: string, problems: PayloadProblem[]): Promise<unknown> {
  const selected = variants.find((o) => o.value === value?.[tag]) ?? variants[0];
  const inner = await buildPayload(selected?.fields ?? [], value ?? {}, path);
  problems.push(...inner.problems);
  return { [tag]: selected?.value, ...inner.payload };
}

// ------------------------------------------------------------------ layout

const SECONDARY_NAMES = new Set(["password", "passwords", "name", "index", "now", "timezone", "precision", "sheet", "dialect", "output", "out", "options", "background", "png_level", "seed"]);

/**
 * Splits fields into the main inputs and the optional settings.
 *
 * Required fields, uploads and text editors are always primary. Optional
 * fields *without* an engine default (e.g. `width` / `height` of a resize)
 * are primary too unless they are bookkeeping (`password`, `name`…) or the
 * form would get crowded; everything with a default is an option.
 */
export function partitionFields(fields: FieldSchema[], preset?: Record<string, unknown>): { primary: FieldSchema[]; options: FieldSchema[]; hidden: FieldSchema[] } {
  const primary: FieldSchema[] = [];
  const options: FieldSchema[] = [];
  const hidden: FieldSchema[] = [];
  const hasMode = fields.some((f) => f.name === "mode" || f.name === "strategy");

  for (const f of fields) {
    if (isAutoField(f) || isDerivedName(f, fields)) {
      hidden.push(f);
      continue;
    }
    const presetted = preset && preset[f.name] !== undefined;
    const isInput = f.kind === "file" || f.kind === "files" || f.kind === "textarea";
    const isCoreSelector = ["mode", "strategy", "algorithm", "type", "action", "method", "level"].includes(f.name);
    const isModeParameter = hasMode && (f.name === "ranges" || f.name === "range" || f.name === "n" || f.name === "chunk_size");
    const mainParameter =
      (f.default === undefined || isCoreSelector || isModeParameter) &&
      !SECONDARY_NAMES.has(f.name) &&
      ["number", "integer", "text", "select", "color", "numbers", "matrix", "tags"].includes(f.kind);

    if (!presetted && (f.required || isInput || mainParameter)) primary.push(f);
    else options.push(f);
  }

  // Ensure logical ordering in primary:
  // 1. Uploads/inputs (file, files)
  // 2. Mode / strategy selectors (immediately after upload!)
  // 3. Mode parameters (ranges, n)
  // 4. Other fields
  primary.sort((a, b) => {
    const rank = (f: FieldSchema) => {
      if (f.kind === "file" || f.kind === "files") return 1;
      if (f.name === "mode" || f.name === "strategy" || f.name === "level") return 2;
      if (f.name === "ranges" || f.name === "range") return 3;
      if (f.name === "n" || f.name === "chunk_size") return 4;
      if (f.kind === "textarea") return 5;
      if (f.required) return 6;
      return 10;
    };
    return rank(a) - rank(b);
  });

  // Keep the main form compact: demote optional parameters when it gets crowded.
  while (primary.length > 8) {
    const idx = primary
      .map((f, i) => (!f.required && !["file", "files", "textarea"].includes(f.kind) && !["mode", "strategy", "level"].includes(f.name) ? i : -1))
      .filter((i) => i >= 0)
      .pop();
    if (idx === undefined) break;
    options.unshift(primary.splice(idx, 1)[0]);
  }
  return { primary, options, hidden };
}

const ACTION_VERBS: Record<string, string> = {
  resize: "Resize", crop: "Crop", rotate: "Rotate", flip: "Flip", merge: "Merge", split: "Split", trim: "Trim", cut: "Cut",
  compress: "Compress", extract: "Extract", repair: "Repair", protect: "Protect", unlock: "Unlock", subset: "Subset",
  simplify: "Simplify", reorder: "Reorder", delete: "Delete", duplicate: "Duplicate", redact: "Redact", annotate: "Annotate",
  stamp: "Stamp", overlay: "Overlay", watermark: "Watermark", grayscale: "Convert", linearize: "Optimise", optimize: "Optimise",
  mute: "Mute", loop: "Loop", concat: "Join", join: "Join", mix: "Mix", reverse: "Reverse", normalize: "Normalise", fade: "Apply",
  speed: "Apply", pitch: "Apply", equalizer: "Apply", adjust: "Apply", transform: "Transform", fill: "Fill", sort: "Sort",
  dedupe: "Remove duplicates", filter: "Filter", flatten: "Flatten", unflatten: "Unflatten", escape: "Escape", unescape: "Unescape",
  minify: "Minify", format: "Format", repair_json: "Repair", diff: "Compare", compare: "Compare", lint: "Lint", validate: "Validate",
  parse: "Parse", inspect: "Inspect", info: "Inspect", detect: "Detect", evaluate: "Calculate", solve: "Solve", plot: "Plot",
  search: "Search", lookup: "Look up", render: "Render", preview: "Preview", thumbnail: "Render", pairing: "Suggest",
  size_check: "Check", char_map: "Show", list: "List", test: "Test", create: "Create", write: "Save", clean: "Clean", cleanup: "Clean",
  wrap: "Wrap", truncate: "Truncate", slug: "Slugify", case: "Convert", rot: "Rotate", hashtags: "Suggest", stats: "Count",
  keyword_density: "Analyze", extract_text: "Extract", to_text: "Extract", hash: "Hash", hmac: "Sign", sign: "Sign", verify: "Verify",
  encrypt: "Encrypt", decrypt: "Decrypt", generate: "Generate", bill: "Calculate", schedule: "Plan", timestamp: "Convert",
  age: "Calculate", countdown: "Calculate", calendar: "Show", zones: "List", cron_next: "Preview", pomodoro: "Plan",
  chart: "Draw", histogram: "Draw", pivot: "Pivot", profile: "Profile", describe: "Describe", outliers: "Find", sample: "Sample",
  frequency: "Count", palette: "Generate", gradient: "Generate", shadow: "Generate", contrast: "Check", simulate: "Simulate",
  subtitle_convert: "Convert", subtitle_shift: "Shift", subtitle_validate: "Validate", gif_from_frames: "Build GIF",
  contact_sheet: "Build", frame_plan: "Plan", timecode: "Convert", capabilities: "Show", transcode: "Run",
};

/** Label of the run button, derived from the action name. */
export function verbFor(action: string, category?: string): string {
  const a = action.toLowerCase();
  if (ACTION_VERBS[a]) return ACTION_VERBS[a];
  const head = a.split("_")[0];
  if (a.startsWith("to_") || a.startsWith("from_") || a.includes("convert") || a.endsWith("_to_gltf") || a.startsWith("glb")) return "Convert";
  if (/encode$|_encode|^encode/.test(a)) return "Encode";
  if (/decode$|_decode|^decode/.test(a)) return "Decode";
  if (/generate|random|password|passphrase|uuid|ulid|nanoid|tone|dtmf|lorem|^qr_|barcode_generate|favicon|og_image|sprite|scale$|gitignore|totp|hotp|totp_uri|rsa_generate/.test(a)) return "Generate";
  if (/inspect|info|metadata|detect|analy|validate|check|stat|profile|lint|compare|describe|count|outlier|signature|permission|strength|explain|password_strength/.test(a)) return "Analyze";
  if (/format|minify|prettify|beautify|sort|clean|repair|optimi[sz]e|compress|normalize/.test(a)) return "Format";
  if (ACTION_VERBS[head]) return ACTION_VERBS[head];
  if (/calc|estimate|loan|mortgage|interest|investment|deposit|ppf|inflation|cagr|roi|npv|irr|discount|markup|margin|break_even|salary|hike|overtime|freelance|currency|tip|tax|gst|itc|tds|capital_gains|professional|reconcile|valuation|unit_economics|payroll|cash_flow|commission|pricing|reorder|turnover|profit|battery|inverter|panel|carbon|appliances|concrete|masonry|plaster|paint|tiles|roofing|steel|rebar|excavation|wall_area|staircase|tank|cost|distance|area|bbox|difference|business_days|week_number|percentage|ratio|fraction|quadratic|linear|numeric_root|probability|combinatorics|factorial|integral|derivative|limit|trigonometry|triangle|matrix|sequence|base_convert|binary_calc|roman|number_to_words|scientific_notation|pixels|shoe_size|paper_size|temperature|cooking|units|bitrate|frames|speed_calculator|resize_calculator|crop_calculator|aspect_ratio|bpm|file_size|prime|gcd|geohash|utm|destination|convert_coordinate/.test(a)) return "Calculate";
  if (category && /calculator|math|finance|construction|solar|tax|everyday|productivity/i.test(category)) return "Calculate";
  return "Run";
}
