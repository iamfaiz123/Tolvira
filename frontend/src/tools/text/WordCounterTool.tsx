import React, { useState, useEffect } from "react";
import { WorkerManager } from "../../services/worker-manager";
import type { ToolDefinition } from "../../types";

interface Props {
  tool: ToolDefinition;
}

interface Analysis {
  words: number;
  chars: number;
  chars_no_spaces: number;
  lines: number;
  sentences: number;
  reading_time_seconds: number;
}

export const WordCounterTool: React.FC<Props> = ({ tool }) => {
  const [text, setText] = useState(
    "Universal online utility platform containing tools for developers, creators, and analysts. High performance WebAssembly engines execute heavy processing entirely inside the browser without uploading sensitive files to remote servers.",
  );
  const [analysis, setAnalysis] = useState<Analysis | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    let cancel = false;
    const runAnalysis = async () => {
      if (!text.trim()) {
        setAnalysis({
          words: 0,
          chars: 0,
          chars_no_spaces: 0,
          lines: 0,
          sentences: 0,
          reading_time_seconds: 0,
        });
        return;
      }

      setLoading(true);
      try {
        const wasmUrl = tool.engine?.url || "/engines/text/1.0.0/text_engine.wasm";
        const workerMgr = WorkerManager.getInstance();
        const result = await workerMgr.executeTask<any, Analysis>(
          "text-engine",
          wasmUrl,
          "analyze",
          { text },
        );

        if (!cancel && result) {
          setAnalysis(result);
        }
      } catch (err) {
        console.error("Text analysis error:", err);
      } finally {
        if (!cancel) setLoading(false);
      }
    };

    const debounceTimer = setTimeout(runAnalysis, 150);
    return () => {
      cancel = true;
      clearTimeout(debounceTimer);
    };
  }, [text, tool.engine?.url]);

  return (
    <div className="rounded-xl border border-stone-200 bg-white p-6 shadow-sm dark:border-stone-800 dark:bg-stone-900">
      <div className="mb-4">
        <h3 className="text-lg font-bold text-stone-900 dark:text-stone-100">
          Real-Time Word & Text Counter
        </h3>
        <p className="text-xs text-stone-500">
          Analyzed with <span className="font-semibold text-teal-600">text-engine.wasm</span> in Web Worker
        </p>
      </div>

      <div className="mb-4 grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-6">
        {[
          { label: "Words", value: analysis?.words ?? 0 },
          { label: "Characters", value: analysis?.chars ?? 0 },
          { label: "No Spaces", value: analysis?.chars_no_spaces ?? 0 },
          { label: "Lines", value: analysis?.lines ?? 0 },
          { label: "Sentences", value: analysis?.sentences ?? 0 },
          {
            label: "Reading Time",
            value: `${analysis?.reading_time_seconds ?? 0}s`,
          },
        ].map((stat, idx) => (
          <div
            key={idx}
            className="rounded-lg border border-stone-100 bg-stone-50 p-3 text-center dark:border-stone-800 dark:bg-stone-800"
          >
            <div className="text-xl font-bold text-teal-800 dark:text-teal-400">
              {stat.value}
            </div>
            <div className="text-xs text-stone-500">{stat.label}</div>
          </div>
        ))}
      </div>

      <div>
        <label className="mb-1.5 block text-xs font-semibold uppercase tracking-wider text-stone-600 dark:text-stone-400">
          Enter Text to Analyze
        </label>
        <textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          rows={6}
          className="w-full rounded-lg border border-stone-300 p-3 text-sm focus:border-teal-500 focus:ring-1 focus:ring-teal-500 dark:border-stone-700 dark:bg-stone-800"
          placeholder="Paste or write your text here for real-time word counting and analysis..."
        />
        {loading && (
          <div className="mt-1 text-right text-xs text-teal-600">Analyzing via WASM...</div>
        )}
      </div>
    </div>
  );
};

export default WordCounterTool;

