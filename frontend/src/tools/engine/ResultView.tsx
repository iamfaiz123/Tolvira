import React, { useEffect, useMemo, useState } from "react";
import { Check, ChevronDown, ChevronRight, Copy, Download, ExternalLink } from "lucide-react";

/** A file returned by an engine (base64 on the wire, never shown to the user). */
export interface OutputFile {
  name: string;
  mime: string;
  base64: string;
  bytes?: number;
}

const FILE_KEYS = ["file", "image", "png", "pdf", "archive", "audio", "video", "thumbnail", "cover"];
const TEXT_KEYS = [
  "text", "result", "output", "formatted", "minified", "encoded", "decoded", "markdown", "html", "css", "sql", "code",
  "json", "yaml", "toml", "xml", "csv", "tsv", "jsonl", "ini", "kml", "wkt", "geojson", "svg", "dbml", "mermaid",
  "unified", "patch", "diff", "token", "jwt", "pem", "public_key_pem", "private_key_pem", "uri", "payload",
  "expression", "cron", "gitignore", "dotenv", "shell", "docker", "typescript", "words", "sentence", "slug",
  "normalized", "explanation", "description", "summary", "query", "url", "hash", "digest", "signature", "ciphertext",
  "plaintext", "base64", "hex", "binary", "base32", "password", "passphrase", "ascii", "morse", "lorem",
];
const LONG_TEXT = 80;

function looksBase64(v: unknown): v is string {
  return typeof v === "string" && v.length > 64 && /^[A-Za-z0-9+/=_-]+$/.test(v.slice(0, 96));
}

/** Finds file outputs (single, `files[]`, `pages[]`, legacy `output`) in a response. */
export function collectOutputs(data: unknown): OutputFile[] {
  const out: OutputFile[] = [];
  const visit = (node: any, depth: number) => {
    if (!node || typeof node !== "object" || depth > 4) return;
    if (Array.isArray(node)) {
      node.forEach((n) => visit(n, depth + 1));
      return;
    }
    for (const key of FILE_KEYS) {
      const v = node[key];
      if (looksBase64(v)) {
        const ext = key === "png" ? "png" : node.extension || node.format || "bin";
        out.push({
          name: node.filename || node.name || `${key}.${ext}`,
          mime: node.mime || (key === "png" ? "image/png" : "application/octet-stream"),
          base64: v,
          bytes: typeof node.bytes === "number" ? node.bytes : undefined,
        });
      }
    }
    for (const key of ["files", "outputs", "pages", "parts", "images", "frames", "items", "results", "sizes", "output", "cover"]) {
      if (node[key] && typeof node[key] === "object") visit(node[key], depth + 1);
    }
  };
  visit(data, 0);
  return out;
}

export function base64ToBlob(b64: string, mime: string): Blob {
  const bin = atob(b64.replace(/-/g, "+").replace(/_/g, "/"));
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return new Blob([bytes], { type: mime });
}

function useBlobUrl(b64: string, mime: string): string {
  const [url, setUrl] = useState("");
  useEffect(() => {
    const u = URL.createObjectURL(base64ToBlob(b64, mime));
    setUrl(u);
    return () => URL.revokeObjectURL(u);
  }, [b64, mime]);
  return url;
}

function formatBytes(n?: number): string {
  if (n === undefined) return "";
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(2)} MB`;
}

export function CopyButton({ text, label }: { text: string; label?: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      type="button"
      onClick={() => {
        navigator.clipboard.writeText(text).then(() => {
          setCopied(true);
          setTimeout(() => setCopied(false), 1500);
        });
      }}
      className="inline-flex items-center gap-1 rounded-md border border-stone-300 px-2 py-1 text-xs text-stone-600 hover:border-teal-500 hover:text-teal-700 dark:border-stone-600 dark:text-stone-300"
    >
      {copied ? <Check size={12} /> : <Copy size={12} />} {copied ? "Copied" : (label ?? "Copy")}
    </button>
  );
}

function downloadText(text: string, name: string, mime = "text/plain") {
  const url = URL.createObjectURL(new Blob([text], { type: mime }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

function FileCard({ file, defaultPreview = true }: { file: OutputFile; defaultPreview?: boolean }) {
  const url = useBlobUrl(file.base64, file.mime);
  const isImage = file.mime.startsWith("image/");
  const isPdf = file.mime === "application/pdf";
  const isText = file.mime.startsWith("text/") || /json|xml|svg|csv/.test(file.mime);
  const [showPreview, setShowPreview] = useState((isImage || isPdf || isText) && defaultPreview);
  return (
    <div className="rounded-lg border border-stone-200 bg-stone-50 p-3 dark:border-stone-700 dark:bg-stone-900 shadow-xs" data-testid="output-file">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="min-w-0">
          <div className="truncate font-mono text-sm font-medium text-stone-800 dark:text-stone-100">{file.name}</div>
          <div className="text-xs text-stone-500">
            {file.mime}
            {file.bytes ? ` · ${formatBytes(file.bytes)}` : ""}
          </div>
        </div>
        <div className="flex items-center gap-2">
          {url && (
            <a
              href={url}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-1 rounded-md border border-stone-300 px-2 py-1 text-xs text-stone-600 hover:border-teal-600 hover:text-teal-700 dark:border-stone-600 dark:text-stone-300 transition"
              title="Open preview in new tab / window"
            >
              <ExternalLink size={12} /> Open tab
            </a>
          )}
          {(isPdf || isText || isImage) && (
            <button
              type="button"
              className="rounded-md border border-stone-300 px-2 py-1 text-xs text-stone-600 hover:border-stone-400 dark:border-stone-600 dark:text-stone-300 transition"
              onClick={() => setShowPreview(!showPreview)}
            >
              {showPreview ? "Hide preview" : "Preview"}
            </button>
          )}
          <a
            href={url}
            download={file.name}
            className="inline-flex items-center gap-1 rounded-md bg-teal-700 px-3 py-1.5 text-xs font-semibold text-white hover:bg-teal-800 transition"
            data-testid="download-file"
          >
            <Download size={14} /> Download
          </a>
        </div>
      </div>
      {showPreview && isImage && url && (
        <img
          src={url}
          alt={file.name}
          className="mt-3 max-h-[75vh] w-auto mx-auto rounded border border-stone-200 bg-[repeating-conic-gradient(#e7e5e4_0_25%,transparent_0_50%)] bg-[length:16px_16px] dark:border-stone-700"
        />
      )}
      {showPreview && isPdf && url && (
        <iframe
          title={file.name}
          src={url}
          className="mt-3 h-[calc(100vh-220px)] min-h-[550px] lg:min-h-[720px] w-full rounded-lg border border-stone-200 bg-white dark:border-stone-700 shadow-sm"
        />
      )}
      {file.mime.startsWith("audio/") && url && <audio controls src={url} className="mt-3 w-full" />}
      {file.mime.startsWith("video/") && url && <video controls src={url} className="mt-3 max-h-[75vh] w-full rounded" />}
      {showPreview && isText && url && (
        <iframe
          title={file.name}
          src={url}
          sandbox=""
          className="mt-3 h-[calc(100vh-260px)] min-h-[450px] lg:min-h-[600px] w-full rounded-lg border border-stone-200 bg-white dark:border-stone-700 shadow-sm"
        />
      )}
    </div>
  );
}

function TextBlock({ label, text, mime }: { label: string; text: string; mime?: string }) {
  const isHtml = label === "html" || (text.trim().startsWith("<") && /<\/(html|body|div|p|table|h\d)>/i.test(text));
  const isSvg = label === "svg" || text.trim().startsWith("<svg");
  const [view, setView] = useState<"preview" | "source">(isHtml || isSvg ? "preview" : "source");
  const ext = label === "markdown" ? "md" : label === "html" ? "html" : label === "svg" ? "svg" : label === "csv" ? "csv" : label === "json" ? "json" : "txt";
  return (
    <div className="rounded-lg border border-stone-200 dark:border-stone-700" data-testid={`output-${label}`}>
      <div className="flex items-center justify-between border-b border-stone-200 bg-stone-50 px-3 py-1.5 dark:border-stone-700 dark:bg-stone-900">
        <span className="text-xs font-semibold uppercase tracking-wider text-stone-600 dark:text-stone-300">{label.replace(/_/g, " ")}</span>
        <div className="flex items-center gap-1.5">
          {(isHtml || isSvg) && (
            <button type="button" className="rounded-md border border-stone-300 px-2 py-1 text-xs text-stone-600 dark:border-stone-600 dark:text-stone-300" onClick={() => setView(view === "preview" ? "source" : "preview")}>
              {view === "preview" ? "Source" : "Preview"}
            </button>
          )}
          <CopyButton text={text} />
          <button type="button" className="inline-flex items-center gap-1 rounded-md border border-stone-300 px-2 py-1 text-xs text-stone-600 dark:border-stone-600 dark:text-stone-300" onClick={() => downloadText(text, `${label}.${ext}`, mime)}>
            <Download size={12} /> Save
          </button>
        </div>
      </div>
      {view === "preview" && isSvg ? (
        <div className="overflow-auto bg-white p-3 dark:bg-stone-950" dangerouslySetInnerHTML={{ __html: text }} />
      ) : view === "preview" && isHtml ? (
        <iframe title={label} srcDoc={text} sandbox="" className="h-[calc(100vh-260px)] min-h-[450px] lg:min-h-[600px] w-full bg-white rounded-b-lg" />
      ) : (
        <pre className="max-h-[calc(100vh-260px)] min-h-[160px] overflow-auto whitespace-pre-wrap break-words p-3 font-mono text-xs text-stone-800 dark:text-stone-100">{text}</pre>
      )}
    </div>
  );
}

function formatValue(v: unknown): string {
  if (v === null || v === undefined) return "—";
  if (typeof v === "number") return Number.isInteger(v) ? v.toLocaleString() : v.toLocaleString(undefined, { maximumFractionDigits: 6 });
  if (typeof v === "boolean") return v ? "Yes" : "No";
  if (typeof v === "string") return v;
  return JSON.stringify(v);
}

function isPrimitive(v: unknown): boolean {
  return v === null || ["string", "number", "boolean"].includes(typeof v);
}

function prettyKey(k: string): string {
  const s = k.replace(/_/g, " ");
  return s.charAt(0).toUpperCase() + s.slice(1);
}

function StatGrid({ entries }: { entries: [string, unknown][] }) {
  return (
    <dl className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3" data-testid="output-stats">
      {entries.map(([k, v]) => (
        <div key={k} className="rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 dark:border-stone-700 dark:bg-stone-900">
          <dt className="text-[11px] font-semibold uppercase tracking-wider text-stone-500">{prettyKey(k)}</dt>
          <dd className={`mt-0.5 break-words text-stone-900 dark:text-stone-100 ${typeof v === "number" ? "text-lg font-semibold tabular-nums" : "text-sm"}`}>{formatValue(v)}</dd>
        </div>
      ))}
    </dl>
  );
}

function Table({ rows }: { rows: Record<string, unknown>[] }) {
  const columns = useMemo(() => {
    const cols: string[] = [];
    rows.slice(0, 50).forEach((r) => Object.keys(r).forEach((k) => !cols.includes(k) && cols.push(k)));
    return cols;
  }, [rows]);
  const shown = rows.slice(0, 200);
  return (
    <div className="overflow-auto rounded-lg border border-stone-200 dark:border-stone-700" data-testid="output-table">
      <table className="min-w-full text-left text-xs">
        <thead className="bg-stone-50 dark:bg-stone-900">
          <tr>
            {columns.map((c) => (
              <th key={c} className="whitespace-nowrap px-3 py-2 font-semibold text-stone-600 dark:text-stone-300">
                {prettyKey(c)}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {shown.map((r, i) => (
            <tr key={i} className="border-t border-stone-100 dark:border-stone-800">
              {columns.map((c) => (
                <td key={c} className="max-w-xs truncate px-3 py-1.5 text-stone-800 dark:text-stone-100" title={formatValue(r[c])}>
                  {formatValue(r[c])}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {rows.length > shown.length && <div className="px-3 py-1.5 text-xs text-stone-500">Showing {shown.length} of {rows.length} rows – download the file or copy the JSON for everything.</div>}
    </div>
  );
}

function Section({ title, children, defaultOpen = true }: { title: string; children: React.ReactNode; defaultOpen?: boolean }) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div>
      <button type="button" className="mb-1.5 flex items-center gap-1 text-xs font-semibold uppercase tracking-wider text-stone-600 dark:text-stone-300" onClick={() => setOpen(!open)}>
        {open ? <ChevronDown size={12} /> : <ChevronRight size={12} />} {title}
      </button>
      {open && children}
    </div>
  );
}

/** Renders one level of a result object with the best widget for each value. */
function ObjectView({ data, depth }: { data: Record<string, unknown>; depth: number }) {
  const stats: [string, unknown][] = [];
  const blocks: React.ReactNode[] = [];
  for (const [k, v] of Object.entries(data)) {
    if (looksBase64(v) && FILE_KEYS.includes(k)) continue; // rendered as file card
    if (typeof v === "string" && (v.length > LONG_TEXT || v.includes("\n") || TEXT_KEYS.includes(k)) && !looksBase64(v)) {
      blocks.push(<TextBlock key={k} label={k} text={v} />);
      continue;
    }
    if (isPrimitive(v)) {
      stats.push([k, v]);
      continue;
    }
    if (Array.isArray(v)) {
      if (v.length === 0) {
        stats.push([k, "none"]);
      } else if (v.every(isPrimitive)) {
        stats.push([k, v.length > 12 ? `${v.slice(0, 12).map(formatValue).join(", ")} … (${v.length})` : v.map(formatValue).join(", ")]);
      } else if (v.every((x) => x && typeof x === "object" && !Array.isArray(x) && Object.values(x as object).every((y) => isPrimitive(y) || (Array.isArray(y) && (y as unknown[]).every(isPrimitive))))) {
        blocks.push(
          <Section key={k} title={`${prettyKey(k)} (${v.length})`}>
            <Table rows={v as Record<string, unknown>[]} />
          </Section>,
        );
      } else if (depth < 2) {
        blocks.push(
          <Section key={k} title={`${prettyKey(k)} (${v.length})`} defaultOpen={v.length <= 5}>
            <div className="space-y-2 border-l-2 border-stone-200 pl-3 dark:border-stone-700">
              {(v as unknown[]).slice(0, 20).map((item, i) => (
                <div key={i}>{item && typeof item === "object" ? <ObjectView data={item as Record<string, unknown>} depth={depth + 1} /> : <span className="text-sm">{formatValue(item)}</span>}</div>
              ))}
            </div>
          </Section>,
        );
      } else {
        blocks.push(<JsonBlock key={k} label={k} value={v} />);
      }
      continue;
    }
    if (v && typeof v === "object") {
      const entries = Object.entries(v as Record<string, unknown>);
      if (entries.length === 0) continue;
      if (depth < 2) {
        blocks.push(
          <Section key={k} title={prettyKey(k)} defaultOpen={depth === 0}>
            <div className="border-l-2 border-stone-200 pl-3 dark:border-stone-700">
              <ObjectView data={v as Record<string, unknown>} depth={depth + 1} />
            </div>
          </Section>,
        );
      } else {
        blocks.push(<JsonBlock key={k} label={k} value={v} />);
      }
    }
  }
  return (
    <div className="space-y-3">
      {stats.length > 0 && <StatGrid entries={stats} />}
      {blocks}
    </div>
  );
}

function JsonBlock({ label, value }: { label: string; value: unknown }) {
  const text = JSON.stringify(value, null, 2);
  return <TextBlock label={label} text={text} mime="application/json" />;
}

export function ResultView({ data }: { data: unknown }) {
  const outputs = useMemo(() => collectOutputs(data), [data]);
  const [showRaw, setShowRaw] = useState(false);
  const raw = useMemo(() => JSON.stringify(data, (_k, v) => (looksBase64(v) && v.length > 200 ? `<${v.length} base64 chars>` : v), 2), [data]);
  return (
    <div className="space-y-4" data-testid="engine-result">
      {outputs.length > 0 && (
        <div className="space-y-3">
          {outputs.map((o, i) => (
            <FileCard key={`${o.name}-${i}`} file={o} defaultPreview={outputs.length === 1 || i === 0} />
          ))}
        </div>
      )}
      {data && typeof data === "object" && !Array.isArray(data) ? (
        <ObjectView data={data as Record<string, unknown>} depth={0} />
      ) : Array.isArray(data) ? (
        <ObjectView data={{ items: data }} depth={0} />
      ) : (
        <StatGrid entries={[["result", data]]} />
      )}
      <div>
        <button type="button" className="text-xs text-stone-500 underline" onClick={() => setShowRaw(!showRaw)} data-testid="toggle-raw">
          {showRaw ? "Hide" : "Show"} raw JSON response
        </button>
        {showRaw && (
          <div className="mt-2">
            <TextBlock label="json" text={raw} mime="application/json" />
          </div>
        )}
      </div>
    </div>
  );
}
