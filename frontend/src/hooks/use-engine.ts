import { useState, useEffect, useRef } from "react";
import { EngineLoader } from "../services/engine-loader";
import type {
  EngineProgress,
  WasmEngineInstance,
} from "../types/engine";

export interface UseEngineResult {
  engine: WasmEngineInstance | null;
  loading: boolean;
  progress: EngineProgress | null;
  error: string | null;
  reload: () => void;
}

export function useEngine(
  engineId?: string,
  version: string = "1.0.0",
  wasmUrl?: string,
  expectedHash?: string,
): UseEngineResult {
  const [engine, setEngine] = useState<WasmEngineInstance | null>(null);
  const [loading, setLoading] = useState<boolean>(false);
  const [progress, setProgress] = useState<EngineProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [versionKey, setVersionKey] = useState(0);

  const abortRef = useRef<AbortController | null>(null);

  useEffect(() => {
    if (!engineId || !wasmUrl) {
      setEngine(null);
      setLoading(false);
      return;
    }

    let isMounted = true;
    abortRef.current?.abort();
    abortRef.current = new AbortController();

    setLoading(true);
    setError(null);

    const loader = EngineLoader.getInstance();
    loader
      .loadEngine(engineId, version, wasmUrl, {
        expectedHash,
        signal: abortRef.current.signal,
        onProgress: (p) => {
          if (isMounted) setProgress(p);
        },
      })
      .then((inst) => {
        if (isMounted) {
          setEngine(inst);
          setLoading(false);
        }
      })
      .catch((err) => {
        if (isMounted) {
          setError(err?.message || String(err));
          setLoading(false);
        }
      });

    return () => {
      isMounted = false;
      abortRef.current?.abort();
    };
  }, [engineId, version, wasmUrl, expectedHash, versionKey]);

  return {
    engine,
    loading,
    progress,
    error,
    reload: () => setVersionKey((k) => k + 1),
  };
}

