import { useState, useCallback } from "react";
import { WorkerManager } from "../services/worker-manager";
import { EngineLoader } from "../services/engine-loader";
import type { ToolDefinition } from "../types";

export interface ToolRunOptions {
  useWorker?: boolean;
  timeoutMs?: number;
  onProgress?: (percentage: number, message?: string) => void;
}

export function useToolRunner(tool: ToolDefinition) {
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<number>(0);

  const run = useCallback(
    async <P = any, R = any>(
      action: string,
      payload: P,
      options?: ToolRunOptions,
    ): Promise<R> => {
      setRunning(true);
      setError(null);
      setProgress(0);

      const useWorker = options?.useWorker ?? true;
      const engine = tool.engine;
      const wasmUrl = engine?.url;

      try {
        if (!engine || !wasmUrl) {
          throw new Error(`Tool '${tool.name}' has no configured engine.`);
        }

        if (useWorker) {
          const workerMgr = WorkerManager.getInstance();
          const result = await workerMgr.executeTask<P, R>(
            engine.id,
            wasmUrl,
            action,
            payload,
            {
              timeoutMs: options?.timeoutMs,
              expectedHash: engine.hash,
              onProgress: (pct, msg) => {
                setProgress(pct);
                options?.onProgress?.(pct, msg);
              },
            },
          );
          return result;
        } else {
          const loader = EngineLoader.getInstance();
          const instance = await loader.loadEngine(
            engine.id,
            engine.version,
            wasmUrl,
            { expectedHash: engine.hash },
          );
          const result = await instance.execute(action, payload);
          if (result && result.success === false) {
            throw new Error(result.error || "Tool execution failed");
          }
          return (result?.data ?? result) as R;
        }
      } catch (err: any) {
        const msg = err?.message || String(err);
        setError(msg);
        throw err;
      } finally {
        setRunning(false);
      }
    },
    [tool],
  );

  return { run, running, error, progress };
}

