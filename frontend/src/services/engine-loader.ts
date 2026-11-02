import { CacheManager } from "./cache-manager";
import type {
  EngineLoadingState,
  EngineProgress,
  WasmEngineInstance,
} from "../types/engine";

export type ProgressCallback = (progress: EngineProgress) => void;

export class EngineLoader {
  private static instance: EngineLoader;
  private cacheManager: CacheManager;
  private initializedEngines: Map<string, WasmEngineInstance> = new Map();
  private pendingLoads: Map<string, Promise<WasmEngineInstance>> = new Map();

  private constructor() {
    this.cacheManager = CacheManager.getInstance();
  }

  public static getInstance(): EngineLoader {
    if (!EngineLoader.instance) {
      EngineLoader.instance = new EngineLoader();
    }
    return EngineLoader.instance;
  }

  /**
   * Loads and instantiates a WASM engine on demand.
   * Handles caching, progress reporting, hash verification, and cancellation.
   */
  public async loadEngine(
    engineId: string,
    version: string,
    wasmUrl: string,
    options?: {
      expectedHash?: string;
      onProgress?: ProgressCallback;
      signal?: AbortSignal;
    },
  ): Promise<WasmEngineInstance> {
    const key = `${engineId}:${version}`;

    // 1. Reuse existing instance
    if (this.initializedEngines.has(key)) {
      return this.initializedEngines.get(key)!;
    }

    // 2. Reuse in-flight loading promise
    if (this.pendingLoads.has(key)) {
      return this.pendingLoads.get(key)!;
    }

    const loadPromise = this.doLoadEngine(
      engineId,
      version,
      wasmUrl,
      options,
    ).finally(() => {
      this.pendingLoads.delete(key);
    });

    this.pendingLoads.set(key, loadPromise);
    return loadPromise;
  }

  private async doLoadEngine(
    engineId: string,
    version: string,
    wasmUrl: string,
    options?: {
      expectedHash?: string;
      onProgress?: ProgressCallback;
      signal?: AbortSignal;
    },
  ): Promise<WasmEngineInstance> {
    const report = (
      state: EngineLoadingState,
      loadedBytes: number = 0,
      totalBytes: number = 0,
      error?: string,
    ) => {
      if (options?.onProgress) {
        const percentage = totalBytes > 0 ? Math.round((loadedBytes / totalBytes) * 100) : 0;
        options.onProgress({
          engineId,
          state,
          loadedBytes,
          totalBytes,
          percentage,
          error,
        });
      }
    };

    const key = `${engineId}:${version}`;

    try {
      // 1. Try cache first
      report("fetching", 0, 0);
      let buffer = await this.cacheManager.getEngine(engineId, version);

      if (!buffer) {
        // Fetch from network with progress tracking
        const response = await fetch(wasmUrl, { signal: options?.signal });
        if (!response.ok) {
          throw new Error(
            `Failed to download engine '${engineId}' (${response.status} ${response.statusText})`,
          );
        }

        const contentLength = Number(response.headers.get("Content-Length")) || 0;
        if (!response.body) {
          buffer = await response.arrayBuffer();
        } else {
          // Stream reading for progress reporting
          const reader = response.body.getReader();
          const chunks: Uint8Array[] = [];
          let receivedBytes = 0;

          while (true) {
            const { done, value } = await reader.read();
            if (done) break;
            chunks.push(value);
            receivedBytes += value.length;
            report("fetching", receivedBytes, contentLength);
          }

          const combined = new Uint8Array(receivedBytes);
          let offset = 0;
          for (const chunk of chunks) {
            combined.set(chunk, offset);
            offset += chunk.length;
          }
          buffer = combined.buffer;
        }

        // Cache the downloaded binary
        await this.cacheManager.putEngine(engineId, version, buffer);
      } else {
        report("fetching", buffer.byteLength, buffer.byteLength);
      }

      // 2. Hash integrity check
      if (options?.expectedHash && options.expectedHash !== "unverified") {
        const actualHash = await this.cacheManager.computeHash(buffer);
        if (actualHash !== options.expectedHash && actualHash !== "unverified") {
          throw new Error(
            `Integrity verification failed for engine '${engineId}'. Expected: ${options.expectedHash}, Got: ${actualHash}`,
          );
        }
      }

      // 3. Compile & Instantiate
      report("compiling", buffer.byteLength, buffer.byteLength);
      const wasmModule = await WebAssembly.compile(buffer);

      report("instantiating", buffer.byteLength, buffer.byteLength);
      const instance = await WebAssembly.instantiate(wasmModule, {
        env: {
          abort: () => {
            console.error(`[WasmEngine:${engineId}] Abort called from WASM.`);
          },
        },
      });

      const exports = instance.exports as Record<string, any>;

      // 4. Wrap with typed execute helper
      const engineInstance: WasmEngineInstance = {
        id: engineId,
        version,
        instance,
        module: wasmModule,
        exports,
        execute: async (action: string, payload: any) => {
          return this.executeOnWasm(exports, action, payload);
        },
      };

      this.initializedEngines.set(key, engineInstance);
      report("ready", buffer.byteLength, buffer.byteLength);
      return engineInstance;
    } catch (err: any) {
      const errorMsg = err?.message || String(err);
      report("error", 0, 0, errorMsg);
      throw err;
    }
  }

  private executeOnWasm(
    exports: Record<string, any>,
    action: string,
    payload: any,
  ): any {
    if (typeof exports.execute !== "function" || typeof exports.alloc !== "function") {
      throw new Error("WASM engine does not export required 'alloc' and 'execute' functions.");
    }

    const requestJson = JSON.stringify({ action, payload });
    const encoder = new TextEncoder();
    const encoded = encoder.encode(requestJson);

    // Allocate buffer in WASM memory
    const ptr = exports.alloc(encoded.length);
    if (!ptr) {
      throw new Error("Failed to allocate linear memory in WASM engine.");
    }

    try {
      const memory = exports.memory as WebAssembly.Memory;
      const memBuffer = new Uint8Array(memory.buffer);
      memBuffer.set(encoded, ptr);

      // Execute action
      const resultPtr = exports.execute(ptr, encoded.length);
      const resultLen = typeof exports.result_len === "function" ? exports.result_len() : 0;

      if (!resultPtr || resultLen === 0) {
        return { success: false, error: "Empty result returned from WASM engine." };
      }

      const resultBytes = new Uint8Array(memory.buffer, resultPtr, resultLen);
      const resultJson = new TextDecoder().decode(resultBytes);
      return JSON.parse(resultJson);
    } finally {
      if (typeof exports.dealloc === "function") {
        exports.dealloc(ptr, encoded.length);
      }
    }
  }

  /**
   * Releases an initialized engine to free memory.
   */
  public releaseEngine(engineId: string, version: string): void {
    const key = `${engineId}:${version}`;
    this.initializedEngines.delete(key);
  }
}

