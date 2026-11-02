import React from "react";

interface Props {
  onInsert: (token: string) => void;
  onClear: () => void;
  onBackspace: () => void;
}

const ROWS: { label: string; token: string; kind?: "op" | "fn" }[][] = [
  [
    { label: "sin", token: "sin(", kind: "fn" },
    { label: "cos", token: "cos(", kind: "fn" },
    { label: "tan", token: "tan(", kind: "fn" },
    { label: "ln", token: "ln(", kind: "fn" },
    { label: "log", token: "log10(", kind: "fn" },
    { label: "√", token: "sqrt(", kind: "fn" },
    { label: "xʸ", token: "^", kind: "op" },
    { label: "n!", token: "!", kind: "op" },
  ],
  [
    { label: "7", token: "7" },
    { label: "8", token: "8" },
    { label: "9", token: "9" },
    { label: "÷", token: " / ", kind: "op" },
    { label: "(", token: "(", kind: "op" },
    { label: ")", token: ")", kind: "op" },
    { label: "π", token: "pi", kind: "fn" },
    { label: "e", token: "e", kind: "fn" },
  ],
  [
    { label: "4", token: "4" },
    { label: "5", token: "5" },
    { label: "6", token: "6" },
    { label: "×", token: " * ", kind: "op" },
    { label: "%", token: " % ", kind: "op" },
    { label: "abs", token: "abs(", kind: "fn" },
    { label: "exp", token: "exp(", kind: "fn" },
    { label: "x", token: "x", kind: "fn" },
  ],
  [
    { label: "1", token: "1" },
    { label: "2", token: "2" },
    { label: "3", token: "3" },
    { label: "−", token: " - ", kind: "op" },
    { label: "0", token: "0" },
    { label: ".", token: "." },
    { label: "+", token: " + ", kind: "op" },
    { label: ",", token: ", ", kind: "op" },
  ],
];

/** On-screen calculator keys that append tokens to an expression field. */
export function Keypad({ onInsert, onClear, onBackspace }: Props) {
  return (
    <div className="mt-2 rounded-lg border border-stone-200 bg-stone-50 p-2 dark:border-stone-700 dark:bg-stone-900" data-testid="keypad">
      <div className="grid grid-cols-8 gap-1">
        {ROWS.flat().map((k) => (
          <button
            key={k.label}
            type="button"
            onClick={() => onInsert(k.token)}
            className={`rounded-md py-2 text-sm font-medium transition-colors active:scale-95 ${
              k.kind === "op" ? "bg-teal-100 text-teal-900 hover:bg-teal-200 dark:bg-teal-900 dark:text-teal-100" : k.kind === "fn" ? "bg-stone-200 text-stone-700 hover:bg-stone-300 dark:bg-stone-700 dark:text-stone-200" : "bg-white text-stone-900 shadow-sm hover:bg-stone-100 dark:bg-stone-800 dark:text-stone-100"
            }`}
          >
            {k.label}
          </button>
        ))}
        <button type="button" onClick={onBackspace} className="col-span-4 rounded-md bg-amber-100 py-2 text-sm font-medium text-amber-900 hover:bg-amber-200 dark:bg-amber-900 dark:text-amber-100">
          ⌫ Backspace
        </button>
        <button type="button" onClick={onClear} className="col-span-4 rounded-md bg-red-100 py-2 text-sm font-medium text-red-900 hover:bg-red-200 dark:bg-red-900 dark:text-red-100">
          C Clear
        </button>
      </div>
    </div>
  );
}
