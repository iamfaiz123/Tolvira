import React, { useCallback, useEffect, useMemo, useState } from "react";
import { AlertTriangle, ChevronDown, ChevronRight, Loader2, Play, RotateCcw, Settings2 } from "lucide-react";
import { useToolRunner } from "../../hooks/use-tool-runner";
import { getActionSchema, listActions, type ActionSchema, type FieldSchema } from "../../registry/engine-schemas";
import { getEngineToolMapping } from "../../registry/tool-registry";
import type { ToolDefinition } from "../../types";
import { Doc, FieldRenderer, inputClass, isWide } from "./fields";
import { buildPayload, entropy, initialValues, isFieldDisabled, partitionFields, verbFor, type FormValues, type PayloadProblem } from "./form-model";
import { ResultView } from "./ResultView";

interface Props {
  tool: ToolDefinition;
}

const ERROR_TITLES: Record<string, string> = {
  E_EMPTY_INPUT: "Nothing to process",
  E_INVALID_INPUT: "Invalid input",
  E_UNSUPPORTED_FORMAT: "Unsupported file format",
  E_MALFORMED: "The file looks damaged",
  E_ENCRYPTED: "The document is password protected",
  E_WRONG_PASSWORD: "Wrong password",
  E_LIMIT_EXCEEDED: "Input too large",
  E_NOT_FOUND: "Not found",
  E_UNSUPPORTED: "Not available in the browser",
  E_ENTROPY_REQUIRED: "Randomness required",
  E_BAD_REQUEST: "Request rejected",
  E_INTERNAL: "Engine error",
};

function FieldGrid({ fields, values, onChange, engineId }: { fields: FieldSchema[]; values: FormValues; onChange: (name: string, v: unknown) => void; engineId: string }) {
  return (
    <div className="grid gap-4 md:grid-cols-2">
      {fields.map((f) => {
        const { disabled, reason } = isFieldDisabled(f, values, fields);
        return (
          <div key={f.name} className={isWide(f) ? "md:col-span-2" : ""}>
            <FieldRenderer
              field={f}
              value={values[f.name]}
              onChange={(v) => onChange(f.name, v)}
              siblings={fields}
              engineId={engineId}
              disabled={disabled}
              disabledReason={reason}
            />
          </div>
        );
      })}
    </div>
  );
}

/**
 * Generic UI for every engine-backed tool. The form is generated from
 * `shared/data/engine-schemas.json` (derived from the Rust action enums), so
 * users get file pickers, number inputs, switches and selects – never raw
 * JSON or base64. Results are rendered as downloads, previews, tables and
 * stat cards.
 */
export const EngineToolRunner: React.FC<Props> = ({ tool }) => {
  const mapping = useMemo(() => getEngineToolMapping(tool.slug), [tool.slug]);
  const engineId = tool.engine?.id ?? "";
  const { run, running, progress } = useToolRunner(tool);
  const actions = useMemo(() => listActions(engineId), [engineId]);
  const [action, setAction] = useState(mapping?.action ?? actions[0] ?? "");
  const schema: ActionSchema | undefined = useMemo(() => getActionSchema(engineId, action), [engineId, action]);
  const preset = mapping?.action === action ? mapping?.preset : undefined;
  const layout = useMemo(() => partitionFields(schema?.fields ?? [], preset), [schema, preset]);
  const [values, setValues] = useState<FormValues>(() => initialValues(schema?.fields ?? [], preset));
  const compact = (l: { primary: FieldSchema[]; options: FieldSchema[] }) => l.primary.length + l.options.length <= 10;
  const [showOptions, setShowOptions] = useState(compact(layout));
  const [problems, setProblems] = useState<PayloadProblem[]>([]);
  const [result, setResult] = useState<unknown>(null);
  const [error, setError] = useState<{ code?: string; message: string } | null>(null);
  const [elapsed, setElapsed] = useState<number | null>(null);

  useEffect(() => {
    setAction(mapping?.action ?? actions[0] ?? "");
  }, [mapping, actions]);

  useEffect(() => {
    setValues(initialValues(schema?.fields ?? [], preset));
    setProblems([]);
    setResult(null);
    setError(null);
    setShowOptions(compact(partitionFields(schema?.fields ?? [], preset)));
  }, [schema, preset]);

  const update = useCallback((name: string, v: unknown) => {
    setValues((prev) => {
      const next = { ...prev, [name]: v };
      if (name === "mode" && v === "every_n" && (!next.n || Number(next.n) < 1)) {
        next.n = 1;
      }
      return next;
    });
  }, []);

  const execute = useCallback(async () => {
    if (!schema) return;
    setError(null);
    setResult(null);
    const { payload, problems: found } = await buildPayload(schema.fields, values);
    setProblems(found);
    if (found.length > 0) return;
    // Engines have no entropy source of their own: every request carries fresh
    // randomness (ignored by actions that do not need it).
    if (payload.entropy === undefined) payload.entropy = entropy();
    const start = performance.now();
    try {
      const data = await run(action, payload, { timeoutMs: 180_000 });
      setElapsed(performance.now() - start);
      setResult(data);
    } catch (e: unknown) {
      const err = e as Error & { code?: string };
      setElapsed(performance.now() - start);
      setError({ code: err.code, message: err.message || String(e) });
    }
  }, [schema, values, run, action]);

  const reset = () => {
    setValues(initialValues(schema?.fields ?? [], preset));
    setProblems([]);
    setResult(null);
    setError(null);
    setElapsed(null);
  };

  if (!tool.engine) {
    return <div className="rounded-xl border border-stone-200 bg-white p-6 dark:border-stone-800 dark:bg-stone-900">This tool has no engine configured.</div>;
  }

  const verb = verbFor(action, tool.category);
  const hasResult = result !== null || error !== null;

  return (
    <div
      className="rounded-xl border border-stone-200 bg-white p-6 shadow-sm dark:border-stone-800 dark:bg-stone-900"
      data-testid="engine-tool-runner"
      onKeyDown={(e) => {
        if ((e.ctrlKey || e.metaKey) && e.key === "Enter") execute();
      }}
    >
      <div className="mb-5 flex flex-wrap items-start justify-between gap-3">
        <div>
          <h3 className="text-lg font-bold text-stone-900 dark:text-stone-100">{tool.name}</h3>
          {schema?.doc && <Doc text={schema.doc} className="mt-0.5 text-sm text-stone-600 dark:text-stone-300" />}
          <p className="mt-1 text-xs text-stone-500 dark:text-stone-400">
            Runs offline in your browser (<code className="rounded bg-stone-100 px-1 dark:bg-stone-800">{tool.engine.id}</code> v{tool.engine.version}) – files never leave your device.
            {mapping?.status === "partial" && <span className="ml-2 rounded bg-amber-100 px-2 py-0.5 text-[11px] text-amber-800 dark:bg-amber-900 dark:text-amber-100">partial support</span>}
          </p>
          {mapping?.note && <p className="mt-1 text-xs text-stone-500 dark:text-stone-400">{mapping.note}</p>}
        </div>
        {!mapping?.action && actions.length > 1 && (
          <label className="text-xs text-stone-600 dark:text-stone-300">
            Action
            <select className={`${inputClass} mt-1`} value={action} onChange={(e) => setAction(e.target.value)} data-testid="engine-action">
              {actions.map((a) => (
                <option key={a} value={a}>
                  {a}
                </option>
              ))}
            </select>
          </label>
        )}
      </div>

      {!schema ? (
        <div className="rounded-lg border border-amber-200 bg-amber-50 p-3 text-sm text-amber-800">No form schema for action “{action}”. Regenerate `shared/data/engine-schemas.json`.</div>
      ) : (
        <div className={`grid gap-6 ${hasResult ? "lg:grid-cols-2 lg:items-start" : ""}`}>
          <form
            className="space-y-4 min-w-0"
            onSubmit={(e) => {
              e.preventDefault();
              execute();
            }}
          >
            {layout.primary.length > 0 && <FieldGrid fields={layout.primary} values={values} onChange={update} engineId={engineId} />}
            {layout.options.length > 0 && (
              <div className="rounded-lg border border-stone-200 dark:border-stone-700">
                <button type="button" className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm font-semibold text-stone-700 dark:text-stone-200" onClick={() => setShowOptions(!showOptions)} data-testid="toggle-options">
                  {showOptions ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                  <Settings2 size={14} /> Options
                  <span className="text-xs font-normal text-stone-500">({layout.options.length})</span>
                </button>
                {showOptions && (
                  <div className="border-t border-stone-200 p-3 dark:border-stone-700">
                    <FieldGrid fields={layout.options} values={values} onChange={update} engineId={engineId} />
                  </div>
                )}
              </div>
            )}
            {problems.length > 0 && (
              <div className="rounded-lg border border-red-200 bg-red-50 p-3 text-sm text-red-800 dark:border-red-900 dark:bg-red-950 dark:text-red-100" role="alert" data-testid="form-problems">
                <div className="mb-1 flex items-center gap-1 font-semibold">
                  <AlertTriangle size={14} /> Please check the form
                </div>
                <ul className="list-inside list-disc">
                  {problems.map((p, i) => (
                    <li key={i}>
                      <span className="font-medium">{p.path}</span>: {p.message}
                    </li>
                  ))}
                </ul>
              </div>
            )}
            <div className="flex flex-wrap items-center gap-3">
              <button type="submit" disabled={running} className="inline-flex items-center gap-2 rounded-lg bg-teal-700 px-5 py-2.5 text-sm font-semibold text-white shadow-sm transition-all hover:bg-teal-800 active:scale-[0.98] disabled:opacity-60" data-testid="engine-run">
                {running ? <Loader2 size={16} className="animate-spin" /> : <Play size={16} />}
                {running ? `Working… ${progress ? `${progress}%` : ""}` : verb}
              </button>
              <button type="button" onClick={reset} className="inline-flex items-center gap-1 rounded-lg border border-stone-300 px-3 py-2 text-sm text-stone-600 hover:border-stone-400 dark:border-stone-600 dark:text-stone-300" data-testid="engine-reset">
                <RotateCcw size={14} /> Reset
              </button>
              <span className="text-xs text-stone-400">Ctrl/⌘ + Enter</span>
              {elapsed !== null && !running && <span className="text-xs text-stone-500">{elapsed < 1000 ? `${elapsed.toFixed(0)} ms` : `${(elapsed / 1000).toFixed(2)} s`}</span>}
            </div>
          </form>

          {hasResult && (
            <div className="space-y-3 lg:sticky lg:top-4 lg:self-start w-full min-w-0" data-testid="engine-output">
              {error && (
                <div className="rounded-lg border border-red-200 bg-red-50 p-4 text-sm text-red-800 dark:border-red-900 dark:bg-red-950 dark:text-red-100" role="alert" data-testid="engine-error">
                  <div className="mb-1 flex items-center gap-1 font-semibold">
                    <AlertTriangle size={14} /> {ERROR_TITLES[error.code ?? ""] ?? "Something went wrong"}
                  </div>
                  <p>{error.message}</p>
                  {error.code && <p className="mt-1 font-mono text-[11px] opacity-70">{error.code}</p>}
                </div>
              )}
              {result !== null && (
                <>
                  <div className="text-xs font-semibold uppercase tracking-wider text-stone-600 dark:text-stone-400">Result</div>
                  <ResultView data={result} />
                </>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export default EngineToolRunner;
