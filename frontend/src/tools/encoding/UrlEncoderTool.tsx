import React, { useState } from "react";
import type { ToolDefinition } from "../../types";

interface Props {
  tool: ToolDefinition;
}

export const UrlEncoderTool: React.FC<Props> = () => {
  const [input, setInput] = useState("https://example.com/search?query=developer tools&category=WASM & Web Workers");
  const [output, setOutput] = useState("");
  const [copied, setCopied] = useState(false);

  const encode = () => {
    setOutput(encodeURIComponent(input));
  };

  const decode = () => {
    try {
      setOutput(decodeURIComponent(input));
    } catch {
      setOutput("Error: Malformed URI sequence");
    }
  };

  return (
    <div className="rounded-xl border border-stone-200 bg-white p-6 shadow-sm dark:border-stone-800 dark:bg-stone-900">
      <div className="mb-4">
        <h3 className="text-lg font-bold text-stone-900 dark:text-stone-100">
          URL Encoder & Decoder
        </h3>
        <p className="text-xs text-stone-500">
          Percent-encode and decode URL parameters and query strings safely
        </p>
      </div>

      <div className="space-y-4">
        <div>
          <label className="mb-1.5 block text-xs font-semibold uppercase text-stone-600">
            Input URL / String
          </label>
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            rows={4}
            className="w-full rounded-lg border border-stone-300 p-3 text-sm focus:border-teal-500 dark:border-stone-700 dark:bg-stone-800"
          />
        </div>

        <div className="flex gap-2">
          <button
            onClick={encode}
            className="rounded-lg bg-teal-800 px-4 py-2 text-xs font-medium text-white hover:bg-teal-900"
          >
            Encode URI Component
          </button>
          <button
            onClick={decode}
            className="rounded-lg bg-stone-700 px-4 py-2 text-xs font-medium text-white hover:bg-stone-800"
          >
            Decode URI Component
          </button>
        </div>

        <div>
          <div className="mb-1 flex justify-between">
            <label className="text-xs font-semibold uppercase text-stone-600">Result</label>
            {output && (
              <button
                onClick={() => {
                  navigator.clipboard.writeText(output);
                  setCopied(true);
                  setTimeout(() => setCopied(false), 2000);
                }}
                className="text-xs text-teal-600 hover:underline"
              >
                {copied ? "Copied!" : "Copy Result"}
              </button>
            )}
          </div>
          <textarea
            readOnly
            value={output}
            rows={4}
            className="w-full rounded-lg border border-stone-200 bg-stone-50 p-3 font-mono text-xs dark:border-stone-800 dark:bg-stone-950"
          />
        </div>
      </div>
    </div>
  );
};

export default UrlEncoderTool;

