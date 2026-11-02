import type {
  CategoryDefinition,
  EngineManifest,
  ToolDefinition,
} from "../types";

export interface ToolListResponse {
  tools: ToolDefinition[];
  total: number;
  page: number;
  pageSize: number;
  categories: CategoryDefinition[];
}

export class ApiClient {
  private static instance: ApiClient;
  private baseUrl: string;

  private constructor() {
    this.baseUrl = (import.meta.env.VITE_API_URL as string) || "http://localhost:5001";
  }

  public static getInstance(): ApiClient {
    if (!ApiClient.instance) {
      ApiClient.instance = new ApiClient();
    }
    return ApiClient.instance;
  }

  public setBaseUrl(url: string): void {
    this.baseUrl = url.replace(/\/$/, "");
  }

  public async getHealth(): Promise<{ status: string; tools_indexed: number; engines_available: string[] }> {
    const res = await fetch(`${this.baseUrl}/api/v1/health`);
    if (!res.ok) throw new Error(`Health check failed: ${res.status}`);
    return res.json();
  }

  public async getCategories(): Promise<CategoryDefinition[]> {
    const res = await fetch(`${this.baseUrl}/api/v1/categories`);
    if (!res.ok) throw new Error(`Failed to fetch categories: ${res.status}`);
    return res.json();
  }

  public async getTools(params?: {
    category?: string;
    query?: string;
    status?: string;
    page?: number;
    pageSize?: number;
  }): Promise<ToolListResponse> {
    const query = new URLSearchParams();
    if (params?.category) query.set("category", params.category);
    if (params?.query) query.set("query", params.query);
    if (params?.status) query.set("status", params.status);
    if (params?.page) query.set("page", String(params.page));
    if (params?.pageSize) query.set("page_size", String(params.pageSize));

    const url = `${this.baseUrl}/api/v1/tools?${query.toString()}`;
    const res = await fetch(url);
    if (!res.ok) throw new Error(`Failed to fetch tools: ${res.status}`);
    return res.json();
  }

  public async getToolById(identifier: string | number): Promise<ToolDefinition> {
    const res = await fetch(`${this.baseUrl}/api/v1/tools/${identifier}`);
    if (!res.ok) throw new Error(`Tool '${identifier}' not found: ${res.status}`);
    return res.json();
  }

  public async getEngineManifest(engineId: string): Promise<EngineManifest> {
    const res = await fetch(`${this.baseUrl}/api/v1/engines/${engineId}`);
    if (!res.ok) throw new Error(`Engine '${engineId}' not found: ${res.status}`);
    return res.json();
  }
}

export const apiClient = ApiClient.getInstance();

