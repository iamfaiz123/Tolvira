/**
 * Version-aware caching system for WASM binaries and static engine assets.
 * Uses the browser Cache API with an in-memory fallback.
 */

const CACHE_NAME_PREFIX = "dev-toolbox-engines-v";

export class CacheManager {
  private static instance: CacheManager;
  private memoryCache: Map<string, ArrayBuffer> = new Map();

  private constructor() {}

  public static getInstance(): CacheManager {
    if (!CacheManager.instance) {
      CacheManager.instance = new CacheManager();
    }
    return CacheManager.instance;
  }

  private getCacheName(version: string = "1"): string {
    return `${CACHE_NAME_PREFIX}${version}`;
  }

  /**
   * Retrieves a cached WASM binary ArrayBuffer.
   */
  public async getEngine(engineId: string, version: string): Promise<ArrayBuffer | null> {
    const key = `${engineId}:${version}`;

    // 1. Check in-memory cache first
    if (this.memoryCache.has(key)) {
      return this.memoryCache.get(key)!.slice(0);
    }

    // 2. Check Cache API
    if (typeof caches !== "undefined") {
      try {
        const cache = await caches.open(this.getCacheName(version));
        const response = await cache.match(key);
        if (response) {
          const buffer = await response.arrayBuffer();
          this.memoryCache.set(key, buffer);
          return buffer.slice(0);
        }
      } catch (err) {
        console.warn(`[CacheManager] Failed to read from Cache API for ${key}:`, err);
      }
    }

    return null;
  }

  /**
   * Stores a WASM binary in the cache.
   */
  public async putEngine(
    engineId: string,
    version: string,
    buffer: ArrayBuffer,
  ): Promise<void> {
    const key = `${engineId}:${version}`;
    this.memoryCache.set(key, buffer.slice(0));

    if (typeof caches !== "undefined") {
      try {
        const cache = await caches.open(this.getCacheName(version));
        const response = new Response(buffer.slice(0), {
          headers: {
            "Content-Type": "application/wasm",
            "Cache-Control": "public, max-age=31536000, immutable",
            "X-Engine-Id": engineId,
            "X-Engine-Version": version,
          },
        });
        await cache.put(key, response);
      } catch (err) {
        console.warn(`[CacheManager] Failed to write to Cache API for ${key}:`, err);
      }
    }
  }

  /**
   * Computes SHA-256 hexadecimal hash of an ArrayBuffer.
   */
  public async computeHash(buffer: ArrayBuffer): Promise<string> {
    if (typeof crypto !== "undefined" && crypto.subtle) {
      const digest = await crypto.subtle.digest("SHA-256", buffer);
      return Array.from(new Uint8Array(digest))
        .map((b) => b.toString(16).padStart(2, "0"))
        .join("");
    }
    // Fallback simple checksum if WebCrypto is unavailable
    return "unverified";
  }

  /**
   * Removes cached entry for a given engine version.
   */
  public async invalidate(engineId: string, version: string): Promise<boolean> {
    const key = `${engineId}:${version}`;
    this.memoryCache.delete(key);

    if (typeof caches !== "undefined") {
      try {
        const cache = await caches.open(this.getCacheName(version));
        return await cache.delete(key);
      } catch {
        return false;
      }
    }
    return true;
  }

  /**
   * Clears old cache versions.
   */
  public async purgeOldCaches(currentVersion: string): Promise<void> {
    if (typeof caches === "undefined") return;
    try {
      const currentCache = this.getCacheName(currentVersion);
      const keys = await caches.keys();
      for (const key of keys) {
        if (key.startsWith(CACHE_NAME_PREFIX) && key !== currentCache) {
          await caches.delete(key);
        }
      }
    } catch (err) {
      console.warn("[CacheManager] Error purging old caches:", err);
    }
  }
}

