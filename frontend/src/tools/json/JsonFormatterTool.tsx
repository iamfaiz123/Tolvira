import React, { useState } from "react";
import { WorkerManager } from "../../services/worker-manager";
import type { ToolDefinition } from "../../types";

interface Props {
  tool: ToolDefinition;
}

export const JsonFormatterTool: React.FC<Props> = ({ tool }) => {
  const [input, setInput] = useState(
    '{"name":"Universal Utility Platform","features":["WASM","Web Workers","Lightweight Frontend"],"version":1}',
  );
  const [output, setOutput] = useState("");
  const [indent, setIndent] = useState(2);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const format = async (indentSize: number) => {
    setError(null);
    try {
      const wasmUrl = tool.engine?.url || "/engines/text/1.0.0/text_engine.wasm";
      const workerMgr = WorkerManager.getInstance();
      const res: any = await workerMgr.executeTask("text-engine", wasmUrl, "format_json", {
        text: input,
        indent: indentSize,
      });

      if (res && res.result !== undefined) {
        setOutput(res.result);
      } else {
        throw new Error(res?.error || "Formatting failed");
      }
    } catch (err: any) {
      setError(err?.message || "Invalid JSON syntax");
    }
  };

  return (
    <div className="rounded-xl border border-stone-200 bg-white p-6 shadow-sm dark:border-stone-800 dark:bg-stone-900">
      <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
        <div>
          <h3 className="text-lg font-bold text-stone-900 dark:text-stone-100">
            High-Speed JSON Formatter & Minifier
          </h3>
          <p className="text-xs text-stone-500">
            Validated & Formatted using <span className="font-semibold text-teal-600">text-engine.wasm</span>
          </p>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={() => format(2)}
            className="rounded-lg bg-teal-800 px-3 py-1.5 text-xs font-medium text-white hover:bg-teal-900"
          >
            Prettify (2 Spaces)
          </button>
          <button
            onClick={() => format(4)}
            className="rounded-lg bg-teal-800 px-3 py-1.5 text-xs font-medium text-white hover:bg-teal-900"
          >
            Prettify (4 Spaces)
          </button>
          <button
            onClick={() => format(0)}
            className="rounded-lg bg-stone-700 px-3 py-1.5 text-xs font-medium text-white hover:bg-stone-800"
          >
            Minify (Compact)
          </button>
        </div>
      </div>

      <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
        <div>
          <label className="mb-1 block text-xs font-semibold uppercase text-stone-600">
            Input JSON
          </label>
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            rows={10}
            className="w-full rounded-lg border border-stone-300 p-3 font-mono text-xs focus:border-teal-500 focus:ring-1 focus:ring-teal-500 dark:border-stone-700 dark:bg-stone-800"
            placeholder="Paste raw JSON here..."
          />
        </div>

        <div>
          <div className="mb-1 flex items-center justify-between">
            <label className="text-xs font-semibold uppercase text-stone-600">
              Formatted Output
            </label>
            {output && (
              <button
                onClick={() => {
                  navigator.clipboard.writeText(output);
                  setCopied(true);
                  setTimeout(() => setCopied(false), 2000);
                }}
                className="text-xs text-teal-600 hover:underline"
              >
                {copied ? "Copied!" : "Copy Output"}
              </button>
            )}
          </div>
          <textarea
            readOnly
            value={output}
            rows={10}
            className="w-full rounded-lg border border-stone-200 bg-stone-50 p-3 font-mono text-xs dark:border-stone-800 dark:bg-stone-950"
            placeholder="Formatted JSON will appear here..."
          />
        </div>
      </div>

      {error && (
        <div className="mt-3 rounded-lg border border-red-200 bg-red-50 p-3 text-xs text-red-700">
          ⚠️ {error}
        </div>
      )}
    </div>
  );
};

export default JsonFormatterTool;

