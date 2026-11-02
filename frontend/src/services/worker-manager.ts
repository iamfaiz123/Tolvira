import type {
  WorkerInMessage,
  WorkerOutMessage,
  WorkerTask,
  WorkerTaskResult,
} from "../types/worker";

interface PendingTask {
  resolve: (value: any) => void;
  reject: (reason?: any) => void;
  onProgress?: (percentage: number, message?: string) => void;
  timeoutId?: any;
}

export class WorkerManager {
  private static instance: WorkerManager;
  private worker: Worker | null = null;
  private readyEngines: Set<string> = new Set();
  private pendingTasks: Map<string, PendingTask> = new Map();
  private initResolvers: Map<string, { resolve: () => void; reject: (err: any) => void }> = new Map();

  private constructor() {}

  public static getInstance(): WorkerManager {
    if (!WorkerManager.instance) {
      WorkerManager.instance = new WorkerManager();
    }
    return WorkerManager.instance;
  }

  private getOrCreateWorker(): Worker {
    if (!this.worker) {
      // Create worker using Vite's native module worker support
      this.worker = new Worker(
        new URL("../workers/engine.worker.ts", import.meta.url),
        { type: "module" },
      );

      this.worker.onmessage = this.handleWorkerMessage.bind(this);
      this.worker.onerror = this.handleWorkerError.bind(this);
    }
    return this.worker;
  }

  private handleWorkerMessage(event: MessageEvent<WorkerOutMessage>): void {
    const msg = event.data;

    switch (msg.type) {
      case "ENGINE_READY": {
        this.readyEngines.add(msg.engineId);
        const resolver = this.initResolvers.get(msg.engineId);
        if (resolver) {
          resolver.resolve();
          this.initResolvers.delete(msg.engineId);
        }
        break;
      }

      case "ENGINE_ERROR": {
        const resolver = this.initResolvers.get(msg.engineId);
        if (resolver) {
          resolver.reject(new Error(msg.error));
          this.initResolvers.delete(msg.engineId);
        }
        break;
      }

      case "TASK_RESULT": {
        const pending = this.pendingTasks.get(msg.result.taskId);
        if (pending) {
          clearTimeout(pending.timeoutId);
          if (msg.result.success) {
            pending.resolve(msg.result.data);
          } else {
            const err = new Error(msg.result.error || "Task failed in worker") as Error & { code?: string };
            err.code = msg.result.errorCode;
            pending.reject(err);
          }
          this.pendingTasks.delete(msg.result.taskId);
        }
        break;
      }

      case "TASK_PROGRESS": {
        const pending = this.pendingTasks.get(msg.progress.taskId);
        if (pending?.onProgress) {
          pending.onProgress(msg.progress.percentage, msg.progress.message);
        }
        break;
      }

      case "TASK_CANCELLED": {
        const pending = this.pendingTasks.get(msg.taskId);
        if (pending) {
          clearTimeout(pending.timeoutId);
          pending.reject(new Error("Task cancelled"));
          this.pendingTasks.delete(msg.taskId);
        }
        break;
      }
    }
  }

  private handleWorkerError(error: ErrorEvent): void {
    console.error("[WorkerManager] Worker encountered unexpected error:", error);

    // Reject all pending tasks due to crash
    for (const [taskId, pending] of this.pendingTasks.entries()) {
      clearTimeout(pending.timeoutId);
      pending.reject(new Error(`Worker crashed: ${error.message}`));
    }
    this.pendingTasks.clear();
    this.readyEngines.clear();

    // Recreate worker on next request
    if (this.worker) {
      this.worker.terminate();
      this.worker = null;
    }
  }

  /**
   * Initializes an engine inside the worker.
   */
  public async ensureEngineReady(
    engineId: string,
    wasmUrl: string,
    expectedHash?: string,
  ): Promise<void> {
    if (this.readyEngines.has(engineId)) return;

    const worker = this.getOrCreateWorker();
    return new Promise((resolve, reject) => {
      this.initResolvers.set(engineId, { resolve, reject });
      const msg: WorkerInMessage = {
        type: "INIT_ENGINE",
        engineId,
        wasmUrl,
        expectedHash,
      };
      worker.postMessage(msg);
    });
  }

  /**
   * Dispatches a task to the Web Worker.
   */
  public async executeTask<T = any, R = any>(
    engineId: string,
    wasmUrl: string,
    action: string,
    payload: T,
    options?: {
      transferables?: Transferable[];
      timeoutMs?: number;
      onProgress?: (percentage: number, message?: string) => void;
      expectedHash?: string;
    },
  ): Promise<R> {
    await this.ensureEngineReady(engineId, wasmUrl, options?.expectedHash);

    const worker = this.getOrCreateWorker();
    const taskId = `task_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`;

    return new Promise<R>((resolve, reject) => {
      const timeoutMs = options?.timeoutMs || 30000;
      const timeoutId = setTimeout(() => {
        this.cancelTask(taskId);
        reject(new Error(`Task '${action}' timed out after ${timeoutMs}ms`));
      }, timeoutMs);

      this.pendingTasks.set(taskId, {
        resolve,
        reject,
        onProgress: options?.onProgress,
        timeoutId,
      });

      const task: WorkerTask<T> = {
        taskId,
        engineId,
        action,
        payload,
      };

      const inMsg: WorkerInMessage = {
        type: "EXECUTE_TASK",
        task,
      };

      if (options?.transferables && options.transferables.length > 0) {
        worker.postMessage(inMsg, options.transferables);
      } else {
        worker.postMessage(inMsg);
      }
    });
  }

  /**
   * Cancels a pending task.
   */
  public cancelTask(taskId: string): void {
    if (this.worker && this.pendingTasks.has(taskId)) {
      const inMsg: WorkerInMessage = {
        type: "CANCEL_TASK",
        taskId,
      };
      this.worker.postMessage(inMsg);
    }
  }

  /**
   * Terminates the worker and frees resources.
   */
  public terminate(): void {
    if (this.worker) {
      this.worker.postMessage({ type: "TERMINATE" } as WorkerInMessage);
      this.worker.terminate();
      this.worker = null;
    }
    this.pendingTasks.clear();
    this.readyEngines.clear();
  }
}

