/**
 * Generic Engine Web Worker.
 * Executes WASM engines in background thread off the main UI.
 */

import type {
  WorkerInMessage,
  WorkerOutMessage,
  WorkerTask,
} from "../types/worker";

interface LoadedEngine {
  instance: WebAssembly.Instance;
  exports: Record<string, any>;
}

const engines: Map<string, LoadedEngine> = new Map();
const activeTasks: Map<string, { cancelled: boolean }> = new Map();

self.onmessage = async (event: MessageEvent<WorkerInMessage>) => {
  const msg = event.data;

  switch (msg.type) {
    case "INIT_ENGINE": {
      try {
        if (!engines.has(msg.engineId)) {
          const response = await fetch(msg.wasmUrl);
          const bytes = await response.arrayBuffer();
          const wasmModule = await WebAssembly.compile(bytes);
          const instance = await WebAssembly.instantiate(wasmModule, {
            env: { abort: () => {} },
          });

          engines.set(msg.engineId, {
            instance,
            exports: instance.exports as Record<string, any>,
          });
        }

        const outMsg: WorkerOutMessage = {
          type: "ENGINE_READY",
          engineId: msg.engineId,
        };
        self.postMessage(outMsg);
      } catch (err: any) {
        const outMsg: WorkerOutMessage = {
          type: "ENGINE_ERROR",
          engineId: msg.engineId,
          error: err?.message || String(err),
        };
        self.postMessage(outMsg);
      }
      break;
    }

    case "EXECUTE_TASK": {
      const task = msg.task;
      activeTasks.set(task.taskId, { cancelled: false });
      const startTime = performance.now();

      try {
        const engine = engines.get(task.engineId);
        if (!engine) {
          throw new Error(`Engine '${task.engineId}' not initialized in worker.`);
        }

        // Check cancellation
        if (activeTasks.get(task.taskId)?.cancelled) {
          self.postMessage({
            type: "TASK_CANCELLED",
            taskId: task.taskId,
          } as WorkerOutMessage);
          activeTasks.delete(task.taskId);
          return;
        }

        const resultData = executeWasm(engine.exports, task.action, task.payload);
        const durationMs = Math.round(performance.now() - startTime);

        const outMsg: WorkerOutMessage = {
          type: "TASK_RESULT",
          result: {
            taskId: task.taskId,
            success: resultData.success !== false,
            data: resultData.data ?? resultData,
            error: resultData.error,
            errorCode: resultData.error_code,
            durationMs,
          },
        };

        self.postMessage(outMsg);
      } catch (err: any) {
        const outMsg: WorkerOutMessage = {
          type: "TASK_RESULT",
          result: {
            taskId: task.taskId,
            success: false,
            error: err?.message || String(err),
            durationMs: Math.round(performance.now() - startTime),
          },
        };
        self.postMessage(outMsg);
      } finally {
        activeTasks.delete(task.taskId);
      }
      break;
    }

    case "CANCEL_TASK": {
      if (activeTasks.has(msg.taskId)) {
        activeTasks.get(msg.taskId)!.cancelled = true;
      }
      break;
    }

    case "TERMINATE": {
      engines.clear();
      activeTasks.clear();
      self.close();
      break;
    }
  }
};

function executeWasm(
  exports: Record<string, any>,
  action: string,
  payload: any,
): any {
  if (typeof exports.execute !== "function" || typeof exports.alloc !== "function") {
    throw new Error("WASM engine does not export 'alloc' and 'execute'");
  }

  const requestJson = JSON.stringify({ action, payload });
  const encoder = new TextEncoder();
  const encoded = encoder.encode(requestJson);

  const ptr = exports.alloc(encoded.length);
  if (!ptr) {
    throw new Error("Failed to allocate memory in WASM worker");
  }

  try {
    const memory = exports.memory as WebAssembly.Memory;
    new Uint8Array(memory.buffer).set(encoded, ptr);

    const resultPtr = exports.execute(ptr, encoded.length);
    const resultLen = typeof exports.result_len === "function" ? exports.result_len() : 0;

    if (!resultPtr || resultLen === 0) {
      return { success: false, error: "Empty result from WASM engine" };
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

