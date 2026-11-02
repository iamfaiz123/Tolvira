export type EngineType = "wasm" | "js" | "webgl" | "webgpu";

export interface EngineManifest {
  id: string;
  name: string;
  version: string;
  type: EngineType;
  url: string;
  glue?: string;
  worker?: string;
  hash?: string;
  size?: number;
  capabilities: string[];
  dependencies: string[];
}

export type EngineLoadingState =
  | "idle"
  | "fetching"
  | "compiling"
  | "instantiating"
  | "ready"
  | "error";

export interface EngineProgress {
  engineId: string;
  state: EngineLoadingState;
  loadedBytes: number;
  totalBytes: number;
  percentage: number;
  error?: string;
}

export interface WasmEngineInstance {
  id: string;
  version: string;
  instance: WebAssembly.Instance;
  module: WebAssembly.Module;
  exports: Record<string, any>;
  execute(action: string, payload: any): Promise<any>;
}

