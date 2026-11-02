import React, { useState } from "react";
import { EngineLoader } from "../../services/engine-loader";
import { WorkerManager } from "../../services/worker-manager";
import type { ToolDefinition } from "../../types";

interface Props {
  tool: ToolDefinition;
}

export const CaseConverterTool: React.FC<Props> = ({ tool }) => {
  const [input, setInput] = useState("Hello world! Universal online utility platform.");
  const [output, setOutput] = useState("");
  const [loading, setLoading] = useState(false);
  const [useWorker, setUseWorker] = useState(true);
  const [statusMsg, setStatusMsg] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const convert = async (targetCase: string) => {
    if (!input.trim()) {
      setOutput("");
      return;
    }

    setLoading(true);
    setStatusMsg(null);
    const start = performance.now();

    try {
      const wasmUrl = tool.engine?.url || "/engines/text/1.0.0/text_engine.wasm";
      let result: any;

      if (useWorker) {
        // Execute via Web Worker
        const workerMgr = WorkerManager.getInstance();
        result = await workerMgr.executeTask(
          "text-engine",
          wasmUrl,
          "convert_case",
          { text: input, target_case: targetCase },
        );
      } else {
        // Execute in main thread via EngineLoader
        const loader = EngineLoader.getInstance();
        const engine = await loader.loadEngine("text-engine", "1.0.0", wasmUrl);
        result = await engine.execute("convert_case", {
          text: input,
          target_case: targetCase,
        });
      }

      const elapsed = (performance.now() - start).toFixed(1);
      if (result && result.result !== undefined) {
        setOutput(result.result);
        setStatusMsg(`Processed in ${elapsed}ms (${useWorker ? "Web Worker" : "Main Thread"})`);
      } else {
        throw new Error(result?.error || "Conversion failed");
      }
    } catch (err: any) {
      console.error("Conversion error:", err);
      setStatusMsg(`Error: ${err.message || String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  const copyToClipboard = () => {
    if (output) {
      navigator.clipboard.writeText(output);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <div className="rounded-xl border border-stone-200 bg-white p-6 shadow-sm dark:border-stone-800 dark:bg-stone-900">
      <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
        <div>
          <h3 className="text-lg font-bold text-stone-900 dark:text-stone-100">
            Interactive Text Case Converter
          </h3>
          <p className="text-xs text-stone-500">
            Powered by <span className="font-semibold text-teal-600">text-engine.wasm</span>
          </p>
        </div>

        <div className="flex items-center gap-2 text-xs">
          <label className="flex items-center gap-1.5 cursor-pointer text-stone-600 dark:text-stone-400">
            <input
              type="checkbox"
              checked={useWorker}
              onChange={(e) => setUseWorker(e.target.checked)}
              className="rounded text-teal-600 focus:ring-teal-500"
            />
            <span>Execute in Web Worker (non-blocking)</span>
          </label>
        </div>
      </div>

      <div className="space-y-4">
        <div>
          <label className="mb-1.5 block text-xs font-semibold uppercase tracking-wider text-stone-600 dark:text-stone-400">
            Input Text
          </label>
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            rows={4}
            className="w-full rounded-lg border border-stone-300 p-3 text-sm focus:border-teal-500 focus:ring-1 focus:ring-teal-500 dark:border-stone-700 dark:bg-stone-800"
            placeholder="Type or paste your text here..."
          />
        </div>

        <div className="flex flex-wrap gap-2">
          {[
            { label: "UPPERCASE", id: "upper" },
            { label: "lowercase", id: "lower" },
            { label: "Title Case", id: "title" },
            { label: "camelCase", id: "camel" },
            { label: "snake_case", id: "snake" },
            { label: "kebab-case", id: "kebab" },
            { label: "PascalCase", id: "pascal" },
          ].map((item) => (
            <button
              key={item.id}
              onClick={() => convert(item.id)}
              disabled={loading}
              className="rounded-lg bg-teal-800 px-3 py-1.5 text-xs font-medium text-white hover:bg-teal-900 active:scale-95 transition-all disabled:opacity-50"
            >
              {item.label}
            </button>
          ))}
        </div>

        {statusMsg && (
          <div className="text-xs font-mono text-stone-500">{statusMsg}</div>
        )}

        <div>
          <div className="mb-1.5 flex items-center justify-between">
            <label className="text-xs font-semibold uppercase tracking-wider text-stone-600 dark:text-stone-400">
              Output Result
            </label>
            {output && (
              <button
                onClick={copyToClipboard}
                className="text-xs font-medium text-teal-600 hover:text-teal-700"
              >
                {copied ? "Copied!" : "Copy to Clipboard"}
              </button>
            )}
          </div>
          <textarea
            readOnly
            value={output}
            rows={4}
            className="w-full rounded-lg border border-stone-200 bg-stone-50 p-3 text-sm font-mono dark:border-stone-800 dark:bg-stone-950"
            placeholder="Converted text will appear here..."
          />
        </div>
      </div>
    </div>
  );
};

export default CaseConverterTool;

