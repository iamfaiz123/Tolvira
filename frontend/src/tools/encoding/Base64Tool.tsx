import React, { useState } from "react";
import type { ToolDefinition } from "../../types";

interface Props {
  tool: ToolDefinition;
}

export const Base64Tool: React.FC<Props> = () => {
  const [input, setInput] = useState("Developer Toolbox Platform");
  const [output, setOutput] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const encode = () => {
    try {
      setError(null);
      const encoded = btoa(unescape(encodeURIComponent(input)));
      setOutput(encoded);
    } catch (err: any) {
      setError(err?.message || "Failed to encode text to Base64");
    }
  };

  const decode = () => {
    try {
      setError(null);
      const decoded = decodeURIComponent(escape(atob(input.trim())));
      setOutput(decoded);
    } catch (err: any) {
      setError(err?.message || "Invalid Base64 sequence");
    }
  };

  return (
    <div className="rounded-xl border border-stone-200 bg-white p-6 shadow-sm dark:border-stone-800 dark:bg-stone-900">
      <div className="mb-4">
        <h3 className="text-lg font-bold text-stone-900 dark:text-stone-100">
          Base64 Encoder & Decoder
        </h3>
        <p className="text-xs text-stone-500">
          Client-side UTF-8 compatible encoding and decoding
        </p>
      </div>

      <div className="space-y-4">
        <div>
          <label className="mb-1.5 block text-xs font-semibold uppercase text-stone-600">
            Input Text / Base64
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
            Encode to Base64
          </button>
          <button
            onClick={decode}
            className="rounded-lg bg-stone-700 px-4 py-2 text-xs font-medium text-white hover:bg-stone-800"
          >
            Decode from Base64
          </button>
        </div>

        {error && (
          <div className="rounded-lg bg-red-50 p-2 text-xs text-red-600">
            ⚠️ {error}
          </div>
        )}

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

export default Base64Tool;

