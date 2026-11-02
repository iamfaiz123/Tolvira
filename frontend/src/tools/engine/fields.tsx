import React, { useEffect, useId, useMemo, useRef, useState } from "react";
import { ArrowDown, ArrowUp, ChevronDown, ChevronRight, FileIcon, Plus, Trash2, Upload, X } from "lucide-react";
import type { FieldOption, FieldSchema } from "../../registry/engine-schemas";
import { initialValues, isDerivedName, listItemDefault, variantDefault, type FormValue, type FormValues } from "./form-model";
import { Keypad } from "./Keypad";

export const inputClass =
  "w-full rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900 placeholder:text-stone-400 focus:border-teal-500 focus:outline-none focus:ring-1 focus:ring-teal-500 disabled:opacity-60 dark:border-stone-700 dark:bg-stone-800 dark:text-stone-100";
const labelClass = "mb-1 block text-xs font-semibold uppercase tracking-wider text-stone-600 dark:text-stone-400";
const helpClass = "mt-1 text-xs text-stone-500 dark:text-stone-400";
const monoClass = "font-mono";

export interface FieldProps {
  field: FieldSchema;
  value: FormValue;
  onChange: (value: FormValue) => void;
  /** Fields defined next to this one (used for derived values). */
  siblings: FieldSchema[];
  engineId: string;
  /** Nesting depth (controls compact layout). */
  depth?: number;
  disabled?: boolean;
  disabledReason?: string;
}

/** Renders doc text, turning `code` spans into styled code. */
export function Doc({ text, className }: { text?: string; className?: string }) {
  if (!text) return null;
  const parts = text.split(/(`[^`]+`)/g);
  return (
    <p className={className ?? helpClass}>
      {parts.map((p, i) =>
        p.startsWith("`") && p.endsWith("`") ? (
          <code key={i} className="rounded bg-stone-100 px-1 text-[11px] text-stone-700 dark:bg-stone-800 dark:text-stone-300">
            {p.slice(1, -1)}
          </code>
        ) : (
          <React.Fragment key={i}>{p}</React.Fragment>
        ),
      )}
    </p>
  );
}

function FieldShell({
  field,
  children,
  inline,
  disabled,
  disabledReason,
}: {
  field: FieldSchema;
  children: React.ReactNode;
  inline?: boolean;
  disabled?: boolean;
  disabledReason?: string;
}) {
  if (inline) {
    return (
      <div className={`flex items-start justify-between gap-3 py-1 transition-opacity ${disabled ? "opacity-50 pointer-events-none" : ""}`}>
        <div>
          <span className="text-sm font-medium text-stone-800 dark:text-stone-200">
            {field.label}
            {field.required && !disabled && <span className="ml-1 text-red-500">*</span>}
          </span>
          <Doc text={disabledReason || field.doc} className="text-xs text-stone-500 dark:text-stone-400" />
        </div>
        {children}
      </div>
    );
  }
  return (
    <div className={`transition-opacity ${disabled ? "opacity-55" : ""}`}>
      <div className="flex items-center justify-between mb-1">
        <label className={labelClass}>
          {field.label}
          {field.required && !disabled && <span className="ml-1 text-red-500">*</span>}
          {field.unit && <span className="ml-1 normal-case tracking-normal text-stone-400">({field.unit})</span>}
        </label>
        {disabled && disabledReason && (
          <span className="text-[10px] font-medium text-amber-800 dark:text-amber-300 bg-amber-50 dark:bg-amber-950/60 border border-amber-200/70 dark:border-amber-800/50 px-2 py-0.5 rounded-full">
            {disabledReason}
          </span>
        )}
      </div>
      {children}
      <Doc text={field.doc} />
    </div>
  );
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(2)} MB`;
}

// ------------------------------------------------------------------- files

function useObjectUrl(file: File | null): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    if (!file || !file.type.startsWith("image/")) {
      setUrl(null);
      return;
    }
    const u = URL.createObjectURL(file);
    setUrl(u);
    return () => URL.revokeObjectURL(u);
  }, [file]);
  return url;
}

export function FileField({ field, value, onChange }: FieldProps) {
  const file = value as File | null;
  const inputRef = useRef<HTMLInputElement>(null);
  const [dragging, setDragging] = useState(false);
  const preview = useObjectUrl(file);
  const id = useId();

  const pick = (list: FileList | null) => {
    const f = list?.[0];
    if (f) onChange(f);
  };

  return (
    <FieldShell field={field}>
      <div
        role="button"
        tabIndex={0}
        onClick={() => inputRef.current?.click()}
        onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && inputRef.current?.click()}
        onDragOver={(e) => {
          e.preventDefault();
          setDragging(true);
        }}
        onDragLeave={() => setDragging(false)}
        onDrop={(e) => {
          e.preventDefault();
          setDragging(false);
          pick(e.dataTransfer.files);
        }}
        className={`flex cursor-pointer items-center gap-3 rounded-lg border-2 border-dashed px-4 py-4 transition-colors ${
          dragging ? "border-teal-500 bg-teal-50 dark:bg-teal-950" : "border-stone-300 hover:border-teal-400 dark:border-stone-700"
        }`}
        data-testid={`file-${field.name}`}
      >
        <input ref={inputRef} id={id} type="file" accept={field.accept} className="hidden" onChange={(e) => pick(e.target.files)} />
        {file ? (
          <>
            {preview ? <img src={preview} alt="" className="h-14 w-14 rounded object-cover" /> : <FileIcon className="h-8 w-8 text-teal-600" />}
            <div className="min-w-0 flex-1">
              <div className="truncate text-sm font-medium text-stone-800 dark:text-stone-100">{file.name}</div>
              <div className="text-xs text-stone-500">
                {formatBytes(file.size)} · {file.type || "unknown type"} · click to replace
              </div>
            </div>
            <button
              type="button"
              className="rounded p-1 text-stone-400 hover:text-red-500"
              onClick={(e) => {
                e.stopPropagation();
                onChange(null);
                if (inputRef.current) inputRef.current.value = "";
              }}
              aria-label="Remove file"
            >
              <X size={16} />
            </button>
          </>
        ) : (
          <>
            <Upload className="h-6 w-6 text-teal-600" />
            <div>
              <div className="text-sm font-medium text-stone-700 dark:text-stone-200">Click to upload or drag & drop</div>
              <div className="text-xs text-stone-500">{field.accept && field.accept !== "*/*" ? field.accept.replace(/,/g, ", ") : "any file"} · stays on your device</div>
            </div>
          </>
        )}
      </div>
    </FieldShell>
  );
}

export function FilesField({ field, value, onChange }: FieldProps) {
  const files = (value as File[]) ?? [];
  const inputRef = useRef<HTMLInputElement>(null);
  const [dragging, setDragging] = useState(false);

  const add = (list: FileList | null) => {
    if (!list) return;
    onChange([...files, ...Array.from(list)]);
  };
  const move = (i: number, dir: -1 | 1) => {
    const j = i + dir;
    if (j < 0 || j >= files.length) return;
    const next = files.slice();
    [next[i], next[j]] = [next[j], next[i]];
    onChange(next);
  };

  return (
    <FieldShell field={field}>
      <div
        role="button"
        tabIndex={0}
        onClick={() => inputRef.current?.click()}
        onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && inputRef.current?.click()}
        onDragOver={(e) => {
          e.preventDefault();
          setDragging(true);
        }}
        onDragLeave={() => setDragging(false)}
        onDrop={(e) => {
          e.preventDefault();
          setDragging(false);
          add(e.dataTransfer.files);
        }}
        className={`flex cursor-pointer items-center gap-3 rounded-lg border-2 border-dashed px-4 py-4 ${
          dragging ? "border-teal-500 bg-teal-50 dark:bg-teal-950" : "border-stone-300 hover:border-teal-400 dark:border-stone-700"
        }`}
        data-testid={`files-${field.name}`}
      >
        <input
          ref={inputRef}
          type="file"
          multiple
          accept={field.accept}
          className="hidden"
          onChange={(e) => {
            add(e.target.files);
            e.target.value = "";
          }}
        />
        <Upload className="h-6 w-6 text-teal-600" />
        <div>
          <div className="text-sm font-medium text-stone-700 dark:text-stone-200">Add files (click or drag & drop)</div>
          <div className="text-xs text-stone-500">{files.length ? `${files.length} selected · order matters` : field.accept && field.accept !== "*/*" ? field.accept.replace(/,/g, ", ") : "any files"}</div>
        </div>
      </div>
      {files.length > 0 && (
        <ul className="mt-2 divide-y divide-stone-200 rounded-lg border border-stone-200 dark:divide-stone-700 dark:border-stone-700">
          {files.map((f, i) => (
            <li key={`${f.name}-${i}`} className="flex items-center gap-2 px-3 py-1.5 text-sm">
              <span className="w-5 text-xs text-stone-400">{i + 1}.</span>
              <span className="min-w-0 flex-1 truncate">{f.name}</span>
              <span className="text-xs text-stone-400">{formatBytes(f.size)}</span>
              <button type="button" className="p-1 text-stone-400 hover:text-stone-700" onClick={() => move(i, -1)} aria-label="Move up" disabled={i === 0}>
                <ArrowUp size={14} />
              </button>
              <button type="button" className="p-1 text-stone-400 hover:text-stone-700" onClick={() => move(i, 1)} aria-label="Move down" disabled={i === files.length - 1}>
                <ArrowDown size={14} />
              </button>
              <button type="button" className="p-1 text-stone-400 hover:text-red-500" onClick={() => onChange(files.filter((_, j) => j !== i))} aria-label="Remove">
                <Trash2 size={14} />
              </button>
            </li>
          ))}
        </ul>
      )}
    </FieldShell>
  );
}

// ----------------------------------------------------------------- scalars

export function TextField({ field, value, onChange, engineId, disabled, disabledReason }: FieldProps) {
  const isExpression = field.name === "expression" && engineId === "math-engine";
  const listId = useId();
  const suggestions = field.suggestions;
  const isDate = engineId === "datetime-engine" && /date|time|^now$|^from$|^to$|^target$|^start$|^end$|^birth/.test(field.name) && !/zone|format|pattern/.test(field.name);
  const placeholder = field.placeholder ?? (isDate ? "e.g. 2024-01-31 or 2024-01-31T10:00" : /colou?r/.test(field.name) ? "#336699" : undefined);
  return (
    <FieldShell field={field} disabled={disabled} disabledReason={disabledReason}>
      <input
        type="text"
        disabled={disabled}
        className={`${inputClass} ${isExpression ? monoClass : ""} ${disabled ? "cursor-not-allowed bg-stone-100 dark:bg-stone-800/60" : ""}`}
        value={value ?? ""}
        maxLength={field.maxLength}
        placeholder={placeholder}
        list={suggestions ? listId : undefined}
        onChange={(e) => onChange(e.target.value)}
        data-testid={`field-${field.name}`}
      />
      {suggestions && (
        <datalist id={listId}>
          {suggestions.map((s) => (
            <option key={s} value={s} />
          ))}
        </datalist>
      )}
    </FieldShell>
  );
}

export function TextAreaField({ field, value, onChange, engineId, disabled, disabledReason }: FieldProps) {
  const isExpression = field.name === "expression" && engineId === "math-engine";
  const code = /json|yaml|xml|toml|csv|sql|css|html|markdown|code|source|patch|schema|expression|wkt|kml|svg|ini|plist|pem|token|certificate|ciphertext/.test(field.name) || /JSON|CSV|YAML|XML|SQL|HTML|source|expression/.test(field.doc);
  const rows = isExpression ? 2 : field.name === "text" || code ? 7 : 4;
  return (
    <FieldShell field={field} disabled={disabled} disabledReason={disabledReason}>
      <textarea
        disabled={disabled}
        className={`${inputClass} ${code ? monoClass : ""} ${disabled ? "cursor-not-allowed bg-stone-100 dark:bg-stone-800/60" : ""} resize-y`}
        rows={rows}
        value={value ?? ""}
        placeholder={field.placeholder ?? (isExpression ? "e.g. (2 + 3) * sqrt(16)" : `Paste or type ${field.label.toLowerCase()}…`)}
        onChange={(e) => onChange(e.target.value)}
        spellCheck={!code}
        data-testid={`field-${field.name}`}
      />
      {isExpression && !disabled && <Keypad onInsert={(token) => onChange(`${value ?? ""}${token}`)} onClear={() => onChange("")} onBackspace={() => onChange(String(value ?? "").slice(0, -1))} />}
    </FieldShell>
  );
}

export function NumberField({ field, value, onChange, disabled, disabledReason }: FieldProps) {
  const step = field.kind === "integer" ? 1 : field.max !== undefined && field.max <= 1 ? 0.01 : field.max !== undefined && field.max <= 10 ? 0.1 : "any";
  const num = value === "" || value === null || value === undefined ? "" : value;
  const useSlider = field.slider && field.min !== undefined && field.max !== undefined;
  return (
    <FieldShell field={field} disabled={disabled} disabledReason={disabledReason}>
      <div className="flex items-center gap-3">
        {useSlider && (
          <input
            type="range"
            disabled={disabled}
            min={field.min}
            max={field.max}
            step={step === "any" ? 0.01 : step}
            value={num === "" ? (field.min ?? 0) : Number(num)}
            onChange={(e) => onChange(Number(e.target.value))}
            className="flex-1 accent-teal-600 disabled:opacity-40"
          />
        )}
        <input
          type="number"
          inputMode="decimal"
          disabled={disabled}
          className={`${inputClass} ${useSlider ? "w-28 flex-none" : ""} ${disabled ? "cursor-not-allowed bg-stone-100 dark:bg-stone-800/60" : ""}`}
          value={num}
          min={field.min}
          max={field.max}
          step={step}
          placeholder={field.placeholder ?? (field.required ? "" : "default")}
          onChange={(e) => onChange(e.target.value === "" ? "" : Number(e.target.value))}
          data-testid={`field-${field.name}`}
        />
      </div>
    </FieldShell>
  );
}

export function BooleanField({ field, value, onChange, disabled, disabledReason }: FieldProps) {
  const on = Boolean(value);
  return (
    <FieldShell field={field} inline disabled={disabled} disabledReason={disabledReason}>
      <button
        type="button"
        role="switch"
        disabled={disabled}
        aria-checked={on}
        onClick={() => onChange(!on)}
        className={`relative mt-0.5 inline-flex h-6 w-11 flex-none items-center rounded-full transition-colors ${on ? "bg-teal-600" : "bg-stone-300 dark:bg-stone-600"} ${disabled ? "cursor-not-allowed opacity-50" : ""}`}
        data-testid={`field-${field.name}`}
      >
        <span className={`inline-block h-5 w-5 transform rounded-full bg-white shadow transition-transform ${on ? "translate-x-5" : "translate-x-0.5"}`} />
      </button>
    </FieldShell>
  );
}

export function SelectField({ field, value, onChange, disabled, disabledReason }: FieldProps) {
  const options = field.options ?? [];
  const known = options.some((o) => o.value === value) || value === "" || value == null;
  const [custom, setCustom] = useState(!known);
  const asButtons = options.length <= 6 && options.every((o) => o.label.length <= 14) && !field.allowCustom;
  if (asButtons) {
    return (
      <FieldShell field={field} disabled={disabled} disabledReason={disabledReason}>
        <div className="flex flex-wrap gap-1.5" role="radiogroup" data-testid={`field-${field.name}`}>
          {!field.required && !field.default && (
            <button
              type="button"
              disabled={disabled}
              onClick={() => onChange("")}
              className={`rounded-lg border px-3 py-1.5 text-xs font-medium ${!value ? "border-teal-600 bg-teal-600 text-white shadow-xs" : "border-stone-300 text-stone-600 hover:border-teal-400 dark:border-stone-600 dark:text-stone-300"} ${disabled ? "cursor-not-allowed opacity-50" : ""}`}
            >
              Default
            </button>
          )}
          {options.map((o) => (
            <button
              key={o.value}
              type="button"
              title={o.doc}
              disabled={disabled}
              onClick={() => onChange(o.value)}
              className={`rounded-lg border px-3 py-1.5 text-xs font-medium ${value === o.value ? "border-teal-600 bg-teal-600 text-white shadow-xs" : "border-stone-300 text-stone-600 hover:border-teal-400 dark:border-stone-600 dark:text-stone-300"} ${disabled ? "cursor-not-allowed opacity-50" : ""}`}
            >
              {o.label}
            </button>
          ))}
        </div>
      </FieldShell>
    );
  }
  return (
    <FieldShell field={field} disabled={disabled} disabledReason={disabledReason}>
      <div className="flex gap-2">
        {!custom ? (
          <select
            disabled={disabled}
            className={`${inputClass} ${disabled ? "cursor-not-allowed bg-stone-100 dark:bg-stone-800/60" : ""}`}
            value={value ?? ""}
            onChange={(e) => onChange(e.target.value)}
            data-testid={`field-${field.name}`}
          >
            {!field.required && !field.default && <option value="">Default</option>}
            {options.map((o) => (
              <option key={o.value} value={o.value} title={o.doc}>
                {o.label}
              </option>
            ))}
          </select>
        ) : (
          <input
            type="text"
            disabled={disabled}
            className={`${inputClass} ${disabled ? "cursor-not-allowed bg-stone-100 dark:bg-stone-800/60" : ""}`}
            value={value ?? ""}
            placeholder="Custom value"
            onChange={(e) => onChange(e.target.value)}
            data-testid={`field-${field.name}`}
          />
        )}
        {field.allowCustom && (
          <button
            type="button"
            disabled={disabled}
            className="flex-none rounded-lg border border-stone-300 px-2 text-xs text-stone-600 dark:border-stone-600 dark:text-stone-300"
            onClick={() => setCustom(!custom)}
          >
            {custom ? "List" : "Other…"}
          </button>
        )}
      </div>
    </FieldShell>
  );
}

export function ColorField({ field, value, onChange, disabled, disabledReason }: FieldProps) {
  const hex = /^#[0-9a-fA-F]{6}$/.test(String(value ?? "")) ? String(value) : "#000000";
  return (
    <FieldShell field={field} disabled={disabled} disabledReason={disabledReason}>
      <div className="flex items-center gap-2">
        <input type="color" disabled={disabled} value={hex} onChange={(e) => onChange(e.target.value)} className="h-9 w-12 cursor-pointer rounded border border-stone-300 bg-transparent p-0.5 dark:border-stone-600 disabled:opacity-50" aria-label={`${field.label} colour`} />
        <input type="text" disabled={disabled} className={`${inputClass} ${monoClass} ${disabled ? "cursor-not-allowed bg-stone-100 dark:bg-stone-800/60" : ""}`} value={value ?? ""} placeholder="#rrggbb, rgb(), hsl() or a name" onChange={(e) => onChange(e.target.value)} data-testid={`field-${field.name}`} />
      </div>
    </FieldShell>
  );
}

export function TagsField({ field, value, onChange, disabled, disabledReason }: FieldProps) {
  const listId = useId();
  return (
    <FieldShell field={field} disabled={disabled} disabledReason={disabledReason}>
      <input
        type="text"
        disabled={disabled}
        className={`${inputClass} ${disabled ? "cursor-not-allowed bg-stone-100 dark:bg-stone-800/60" : ""}`}
        value={value ?? ""}
        list={field.suggestions ? listId : undefined}
        placeholder={field.placeholder ?? "Comma separated values (e.g. 1-3, 4-6)"}
        onChange={(e) => onChange(e.target.value)}
        data-testid={`field-${field.name}`}
      />
      {field.suggestions && !disabled && (
        <>
          <datalist id={listId}>
            {field.suggestions.map((s) => (
              <option key={s} value={s} />
            ))}
          </datalist>
          <div className="mt-1 flex flex-wrap gap-1">
            {field.suggestions.slice(0, 12).map((s) => {
              const selected = String(value ?? "")
                .split(/[\n,]/)
                .map((x) => x.trim())
                .includes(s);
              return (
                <button
                  key={s}
                  type="button"
                  onClick={() => {
                    const items = String(value ?? "")
                      .split(/[\n,]/)
                      .map((x) => x.trim())
                      .filter(Boolean);
                    onChange((selected ? items.filter((x) => x !== s) : [...items, s]).join(", "));
                  }}
                  className={`rounded-full border px-2 py-0.5 text-[11px] ${selected ? "border-teal-600 bg-teal-600 text-white" : "border-stone-300 text-stone-600 dark:border-stone-600 dark:text-stone-300"}`}
                >
                  {s}
                </button>
              );
            })}
          </div>
        </>
      )}
    </FieldShell>
  );
}

export function NumbersField({ field, value, onChange, disabled, disabledReason }: FieldProps) {
  return (
    <FieldShell field={field} disabled={disabled} disabledReason={disabledReason}>
      <input
        type="text"
        inputMode="decimal"
        disabled={disabled}
        className={`${inputClass} ${monoClass} ${disabled ? "cursor-not-allowed bg-stone-100 dark:bg-stone-800/60" : ""}`}
        value={value ?? ""}
        placeholder={field.fixed ? Array.from({ length: field.fixed }, (_, i) => `n${i + 1}`).join(", ") : "e.g. 12, 7.5, 3"}
        onChange={(e) => onChange(e.target.value)}
        data-testid={`field-${field.name}`}
      />
    </FieldShell>
  );
}

export function MatrixField({ field, value, onChange }: FieldProps) {
  return (
    <FieldShell field={field}>
      <textarea className={`${inputClass} ${monoClass}`} rows={4} value={value ?? ""} placeholder={"One row per line, e.g.\n1, 2\n3, 4"} onChange={(e) => onChange(e.target.value)} data-testid={`field-${field.name}`} />
    </FieldShell>
  );
}

export function JsonField({ field, value, onChange }: FieldProps) {
  let problem: string | null = null;
  if (value && String(value).trim()) {
    try {
      JSON.parse(String(value));
    } catch (e) {
      problem = (e as Error).message;
    }
  }
  return (
    <FieldShell field={field}>
      <textarea className={`${inputClass} ${monoClass} ${problem ? "border-red-400" : ""}`} rows={5} value={value ?? ""} placeholder={field.array ? "[ … ]" : "{ … }"} onChange={(e) => onChange(e.target.value)} spellCheck={false} data-testid={`field-${field.name}`} />
      {problem && <p className="mt-1 text-xs text-red-600">{problem}</p>}
    </FieldShell>
  );
}

export function KeyValuesField({ field, value, onChange }: FieldProps) {
  const rows = (value as { key: string; value: string }[]) ?? [];
  const update = (i: number, patch: Partial<{ key: string; value: string }>) => onChange(rows.map((r, j) => (j === i ? { ...r, ...patch } : r)));
  return (
    <FieldShell field={field}>
      <div className="space-y-1.5">
        {rows.map((r, i) => (
          <div key={i} className="flex gap-1.5">
            <input type="text" className={`${inputClass} ${monoClass}`} placeholder="key" value={r.key} onChange={(e) => update(i, { key: e.target.value })} />
            <input type={field.valueKind === "number" ? "number" : "text"} className={inputClass} placeholder="value" value={r.value} onChange={(e) => update(i, { value: e.target.value })} />
            <button type="button" className="p-1 text-stone-400 hover:text-red-500" onClick={() => onChange(rows.filter((_, j) => j !== i))} aria-label="Remove">
              <Trash2 size={14} />
            </button>
          </div>
        ))}
        <button type="button" className="inline-flex items-center gap-1 rounded-lg border border-dashed border-stone-300 px-3 py-1.5 text-xs text-stone-600 hover:border-teal-400 dark:border-stone-600 dark:text-stone-300" onClick={() => onChange([...rows, { key: "", value: "" }])}>
          <Plus size={12} /> Add entry
        </button>
      </div>
    </FieldShell>
  );
}

// ------------------------------------------------------------ composites

export function GroupField(props: FieldProps) {
  const { field, value, onChange, engineId, depth = 0 } = props;
  const [open, setOpen] = useState(!field.collapsible || field.required);
  const inner = (value as FormValues) ?? initialValues(field.fields ?? []);
  return (
    <div className="rounded-lg border border-stone-200 dark:border-stone-700">
      <button type="button" className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm font-semibold text-stone-700 dark:text-stone-200" onClick={() => setOpen(!open)}>
        {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        {field.label}
        {field.doc && <span className="ml-2 truncate text-xs font-normal text-stone-500">{field.doc.replace(/`/g, "")}</span>}
      </button>
      {open && (
        <div className="grid gap-3 border-t border-stone-200 p-3 dark:border-stone-700 md:grid-cols-2">
          {(field.fields ?? []).map((sub) => (
            <FieldRenderer key={sub.name} field={sub} value={inner[sub.name]} onChange={(v) => onChange({ ...inner, [sub.name]: v })} siblings={field.fields ?? []} engineId={engineId} depth={depth + 1} />
          ))}
        </div>
      )}
    </div>
  );
}

function VariantBody({ tag, variants, value, onChange, engineId, depth }: { tag: string; variants: FieldOption[]; value: FormValues; onChange: (v: FormValues) => void; engineId: string; depth: number }) {
  const current = variants.find((o) => o.value === value?.[tag]) ?? variants[0];
  return (
    <div className="space-y-3">
      <select className={inputClass} value={current?.value ?? ""} onChange={(e) => onChange(variantDefault(tag, variants, e.target.value))}>
        {variants.map((o) => (
          <option key={o.value} value={o.value} title={o.doc}>
            {o.label}
          </option>
        ))}
      </select>
      {current?.doc && <Doc text={current.doc} />}
      {(current?.fields ?? []).length > 0 && (
        <div className="grid gap-3 md:grid-cols-2">
          {(current?.fields ?? []).map((sub) => (
            <FieldRenderer key={sub.name} field={sub} value={value?.[sub.name]} onChange={(v) => onChange({ ...value, [sub.name]: v })} siblings={current?.fields ?? []} engineId={engineId} depth={depth + 1} />
          ))}
        </div>
      )}
    </div>
  );
}

export function VariantField(props: FieldProps) {
  const { field, value, onChange, engineId, depth = 0 } = props;
  return (
    <FieldShell field={field}>
      <VariantBody tag={field.tag ?? "op"} variants={field.variants ?? []} value={(value as FormValues) ?? variantDefault(field.tag ?? "op", field.variants ?? [])} onChange={onChange} engineId={engineId} depth={depth} />
    </FieldShell>
  );
}

export function ListField(props: FieldProps) {
  const { field, value, onChange, engineId, depth = 0 } = props;
  const items = (value as FormValues[]) ?? [];
  const item = field.item!;
  const fileFields = item.kind === "group" ? (item.fields ?? []).filter((f) => f.kind === "file") : [];
  const otherFields = item.kind === "group" ? (item.fields ?? []).filter((f) => f.kind !== "file" && !isDerivedName(f, item.fields ?? [])) : [];
  const simpleFileList = item.kind === "group" && fileFields.length === 1 && otherFields.length === 0;

  // A list of `{ file, name }` groups is just a multi-file picker.
  if (simpleFileList) {
    const pseudo: FieldSchema = { ...field, kind: "files", accept: fileFields[0].accept };
    return (
      <FilesField
        {...props}
        field={pseudo}
        value={items.map((it) => it[fileFields[0].name]).filter(Boolean)}
        onChange={(files: File[]) => onChange(files.map((f) => ({ ...initialValues(item.fields ?? []), [fileFields[0].name]: f, name: f.name })))}
      />
    );
  }

  const update = (i: number, v: FormValues) => onChange(items.map((it, j) => (j === i ? v : it)));
  const move = (i: number, dir: -1 | 1) => {
    const j = i + dir;
    if (j < 0 || j >= items.length) return;
    const next = items.slice();
    [next[i], next[j]] = [next[j], next[i]];
    onChange(next);
  };
  return (
    <FieldShell field={field}>
      <div className="space-y-2">
        {items.map((it, i) => (
          <div key={i} className="rounded-lg border border-stone-200 p-3 dark:border-stone-700">
            <div className="mb-2 flex items-center justify-between text-xs text-stone-500">
              <span>
                {field.label} #{i + 1}
              </span>
              <span className="flex gap-1">
                <button type="button" className="p-1 hover:text-stone-800" onClick={() => move(i, -1)} aria-label="Move up" disabled={i === 0}>
                  <ArrowUp size={14} />
                </button>
                <button type="button" className="p-1 hover:text-stone-800" onClick={() => move(i, 1)} aria-label="Move down" disabled={i === items.length - 1}>
                  <ArrowDown size={14} />
                </button>
                <button type="button" className="p-1 hover:text-red-500" onClick={() => onChange(items.filter((_, j) => j !== i))} aria-label="Remove">
                  <Trash2 size={14} />
                </button>
              </span>
            </div>
            {item.kind === "variant" ? (
              <VariantBody tag={item.tag ?? "op"} variants={item.variants ?? []} value={it} onChange={(v) => update(i, v)} engineId={engineId} depth={depth} />
            ) : (
              <div className="grid gap-3 md:grid-cols-2">
                {(item.fields ?? []).map((sub) => (
                  <FieldRenderer key={sub.name} field={sub} value={it[sub.name]} onChange={(v) => update(i, { ...it, [sub.name]: v })} siblings={item.fields ?? []} engineId={engineId} depth={depth + 1} />
                ))}
              </div>
            )}
          </div>
        ))}
        <button type="button" className="inline-flex items-center gap-1 rounded-lg border border-dashed border-stone-300 px-3 py-1.5 text-xs text-stone-600 hover:border-teal-400 dark:border-stone-600 dark:text-stone-300" onClick={() => onChange([...items, listItemDefault(item)])} data-testid={`add-${field.name}`}>
          <Plus size={12} /> Add {field.label.replace(/s$/, "").toLowerCase()}
        </button>
      </div>
    </FieldShell>
  );
}

// ------------------------------------------------------------- dispatcher

export function FieldRenderer(props: FieldProps) {
  const { field } = props;
  switch (field.kind) {
    case "file":
      return <FileField {...props} />;
    case "files":
      return <FilesField {...props} />;
    case "textarea":
      return <TextAreaField {...props} />;
    case "number":
    case "integer":
      return <NumberField {...props} />;
    case "boolean":
      return <BooleanField {...props} />;
    case "select":
      return <SelectField {...props} />;
    case "color":
      return <ColorField {...props} />;
    case "tags":
      return <TagsField {...props} />;
    case "numbers":
      return <NumbersField {...props} />;
    case "matrix":
      return <MatrixField {...props} />;
    case "json":
      return <JsonField {...props} />;
    case "keyvalues":
      return <KeyValuesField {...props} />;
    case "group":
      return <GroupField {...props} />;
    case "list":
      return <ListField {...props} />;
    case "variant":
      return <VariantField {...props} />;
    default:
      return <TextField {...props} />;
  }
}

/** Whether a field takes the full width of the form grid. */
export function isWide(field: FieldSchema): boolean {
  if (field.name === "mode" || field.name === "strategy" || field.name === "level") return true;
  return ["file", "files", "textarea", "matrix", "json", "keyvalues", "group", "list", "variant", "tags"].includes(field.kind);
}

export function useStableFields(fields: FieldSchema[]): FieldSchema[] {
  return useMemo(() => fields, [fields]);
}
