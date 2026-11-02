export interface WorkerTask<T = any> {
  taskId: string;
  engineId: string;
  action: string;
  payload: T;
  transferables?: Transferable[];
}

export interface WorkerTaskResult<R = any> {
  taskId: string;
  success: boolean;
  data?: R;
  error?: string;
  /** Stable machine-readable code from the engine (e.g. `E_INVALID_INPUT`). */
  errorCode?: string;
  durationMs?: number;
}

export interface WorkerProgressMessage {
  taskId: string;
  type: "progress";
  percentage: number;
  message?: string;
}

export type WorkerInMessage =
  | { type: "INIT_ENGINE"; engineId: string; wasmUrl: string; expectedHash?: string }
  | { type: "EXECUTE_TASK"; task: WorkerTask }
  | { type: "CANCEL_TASK"; taskId: string }
  | { type: "TERMINATE" };

export type WorkerOutMessage =
  | { type: "ENGINE_READY"; engineId: string }
  | { type: "ENGINE_ERROR"; engineId: string; error: string }
  | { type: "TASK_RESULT"; result: WorkerTaskResult }
  | { type: "TASK_PROGRESS"; progress: WorkerProgressMessage }
  | { type: "TASK_CANCELLED"; taskId: string };

