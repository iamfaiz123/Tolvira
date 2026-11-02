export type SearchTool = {
  id: number;
  name: string;
  category: string;
  slug: string;
  description: string;
};

type WasmExports = {
  memory: WebAssembly.Memory;
  alloc: (length: number) => number;
  search: (inputPointer: number, inputLength: number) => void;
  result_ptr: () => number;
  result_len: () => number;
};

let wasmPromise: Promise<WasmExports | null> | undefined;
let wasmReady = false;

async function loadSearchWasm(): Promise<WasmExports | null> {
  try {
    const response = await fetch(`${import.meta.env.BASE_URL}search.wasm`);
    if (!response.ok) return null;
    const { instance } = await WebAssembly.instantiateStreaming(response, {});
    const exports = instance.exports as unknown as WasmExports;
    if (
      !exports.memory ||
      !exports.alloc ||
      !exports.search ||
      !exports.result_ptr ||
      !exports.result_len
    ) {
      return null;
    }
    wasmReady = true;
    return exports;
  } catch {
    return null;
  }
}

function getSearchWasm() {
  wasmPromise ??= loadSearchWasm();
  return wasmPromise;
}

function fallbackSearch(
  query: string,
  tools: SearchTool[],
  category?: string,
): number[] {
  const tokens = query.toLocaleLowerCase().trim().split(/\s+/).filter(Boolean);
  return tools
    .filter((tool) => !category || tool.category === category)
    .map((tool) => {
      const title = tool.name.toLocaleLowerCase();
      const categoryName = tool.category.toLocaleLowerCase();
      let score = 0;
      for (const token of tokens) {
        if (title === token) score += 100;
        else if (title.startsWith(token)) score += 80;
        else if (title.includes(token)) score += 60;
        else if (categoryName.includes(token)) score += 20;
        else return { id: tool.id, score: -1 };
      }
      return { id: tool.id, score };
    })
    .filter((result) => result.score >= 0)
    .sort((a, b) => b.score - a.score || a.id - b.id)
    .map((result) => result.id);
}

export async function searchTools(
  query: string,
  tools: SearchTool[],
  category?: string,
): Promise<number[]> {
  const wasm = await getSearchWasm();
  if (!wasm) return fallbackSearch(query, tools, category);

  const bytes = new TextEncoder().encode(JSON.stringify({ query, category, tools }));
  const inputPointer = wasm.alloc(bytes.length);
  new Uint8Array(wasm.memory.buffer, inputPointer, bytes.length).set(bytes);
  wasm.search(inputPointer, bytes.length);
  const outputPointer = wasm.result_ptr();
  const outputLength = wasm.result_len();
  const output = new Uint8Array(wasm.memory.buffer, outputPointer, outputLength);
  return JSON.parse(new TextDecoder().decode(output)) as number[];
}

export function isWasmSearchReady() {
  return wasmReady;
}
