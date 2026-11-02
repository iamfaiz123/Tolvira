export type ExecutionTarget = "client" | "worker" | "server" | "hybrid";
export type ToolStatus = "available" | "coming-soon" | "deprecated";
export type EngineType = "wasm" | "js" | "webgl" | "webgpu";

export interface ToolEngineConfig {
  id: string;
  version: string;
  type: EngineType;
  url: string;
  glue?: string;
  worker?: string;
  hash?: string;
}

export interface ToolUiConfig {
  component?: string;
  lazy?: boolean;
}

export interface ToolDefinition {
  id: number;
  name: string;
  slug: string;
  category: string;
  description: string;
  status: ToolStatus;
  version?: string;
  execution?: ExecutionTarget;
  ui?: ToolUiConfig;
  engine?: ToolEngineConfig;
  features?: string[];
  icon?: string;
}

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
  capabilities?: string[];
  dependencies?: string[];
}

export interface CategoryDefinition {
  name: string;
  slug: string;
  description: string;
  icon?: string;
  count?: number;
}

export interface ToolListResponse {
  tools: ToolDefinition[];
  total: number;
  page: number;
  pageSize: number;
  categories: CategoryDefinition[];
}

export interface HealthResponse {
  status: "ok" | "degraded" | "error";
  version: string;
  timestamp: string;
  enginesAvailable: string[];
}

