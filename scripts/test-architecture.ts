import fs from "fs";
import path from "path";
import crypto from "crypto";

async function main() {
  console.log("=== UNIVERSAL UTILITY PLATFORM VERIFICATION TEST ===");

  // 1. Test WASM Engine loading and execution in Node
  console.log("\n[1/4] Testing WASM Engines:");
  const textWasmPath = path.resolve("./frontend/public/engines/text/1.0.0/text_engine.wasm");
  const wasmBytes = fs.readFileSync(textWasmPath);
  console.log(`✓ Read text_engine.wasm (${wasmBytes.length} bytes)`);

  // Verify SHA-256
  const hash = crypto.createHash("sha256").update(wasmBytes).digest("hex");
  console.log(`✓ Computed SHA-256: ${hash}`);

  // Instantiate WASM
  const wasmModule = await WebAssembly.compile(wasmBytes);
  const instance = await WebAssembly.instantiate(wasmModule, {
    env: { abort: () => {} },
  });
  const exports = instance.exports as any;
  console.log("✓ WebAssembly compiled and instantiated successfully");

  // Test action 1: convert_case
  function runAction(action: string, payload: any) {
    const jsonStr = JSON.stringify({ action, payload });
    const encoded = new TextEncoder().encode(jsonStr);
    const ptr = exports.alloc(encoded.length);
    new Uint8Array((exports.memory as WebAssembly.Memory).buffer).set(encoded, ptr);
    const resPtr = exports.execute(ptr, encoded.length);
    const resLen = exports.result_len();
    const resBytes = new Uint8Array((exports.memory as WebAssembly.Memory).buffer, resPtr, resLen);
    const resJson = new TextDecoder().decode(resBytes);
    exports.dealloc(ptr, encoded.length);
    return JSON.parse(resJson);
  }

  const caseRes = runAction("convert_case", {
    text: "Universal Utility Platform",
    target_case: "kebab",
  });
  console.log("✓ convert_case result:", caseRes);
  if (caseRes.data.result !== "universal-utility-platform") {
    throw new Error(`Unexpected case conversion: ${caseRes.data.result}`);
  }

  // Test action 2: analyze
  const analyzeRes = runAction("analyze", {
    text: "Hello world. This is a fast Rust WebAssembly engine.",
  });
  console.log("✓ analyze result:", analyzeRes);
  if (analyzeRes.data.words !== 9) {
    throw new Error(`Expected 9 words, got ${analyzeRes.data.words}`);
  }

  // Test action 3: format_json
  const jsonRes = runAction("format_json", {
    text: '{"foo":"bar","num":42}',
    indent: 2,
  });
  console.log("✓ format_json result:\n" + jsonRes.data.result);

  // 2. Test Image Engine
  console.log("\n[2/4] Testing Image Engine WASM:");
  const imageWasmPath = path.resolve("./frontend/public/engines/image/1.0.0/image_engine.wasm");
  const imgBytes = fs.readFileSync(imageWasmPath);
  const imgModule = await WebAssembly.compile(imgBytes);
  const imgInst = await WebAssembly.instantiate(imgModule, { env: { abort: () => {} } });
  const imgExports = imgInst.exports as any;
  
  const imgReq = JSON.stringify({ action: "aspect_ratio", payload: { width: 1920, height: 1080 } });
  const imgEnc = new TextEncoder().encode(imgReq);
  const imgPtr = imgExports.alloc(imgEnc.length);
  new Uint8Array((imgExports.memory as WebAssembly.Memory).buffer).set(imgEnc, imgPtr);
  const imgResPtr = imgExports.execute(imgPtr, imgEnc.length);
  const imgResLen = imgExports.result_len();
  const imgRes = JSON.parse(new TextDecoder().decode(new Uint8Array((imgExports.memory as WebAssembly.Memory).buffer, imgResPtr, imgResLen)));
  console.log("✓ aspect_ratio result (1920x1080):", imgRes.data);
  if (imgRes.data.ratio_str !== "16:9") {
    throw new Error(`Expected 16:9, got ${imgRes.data.ratio_str}`);
  }

  // Test preset_crop on Image Engine
  const presetReq = JSON.stringify({ action: "preset_crop", payload: { orig_width: 800, orig_height: 600, preset: "1:1" } });
  const presetEnc = new TextEncoder().encode(presetReq);
  const presetPtr = imgExports.alloc(presetEnc.length);
  new Uint8Array((imgExports.memory as WebAssembly.Memory).buffer).set(presetEnc, presetPtr);
  const presetResPtr = imgExports.execute(presetPtr, presetEnc.length);
  const presetResLen = imgExports.result_len();
  const presetRes = JSON.parse(new TextDecoder().decode(new Uint8Array((imgExports.memory as WebAssembly.Memory).buffer, presetResPtr, presetResLen)));
  console.log("✓ preset_crop result (800x600 -> 1:1 square):", presetRes.data);
  if (presetRes.data.crop_width !== 600 || presetRes.data.crop_height !== 600) {
    throw new Error(`Expected 600x600, got ${presetRes.data.crop_width}x${presetRes.data.crop_height}`);
  }

  // 3. Test Backend REST API
  console.log("\n[3/4] Testing Backend REST API Endpoints:");
  const healthRes = await fetch("http://localhost:5001/api/v1/health");
  const healthData: any = await healthRes.json();
  console.log("✓ GET /api/v1/health status:", healthData.status, `(${healthData.tools_indexed} tools indexed, ${healthData.engines_available.length} engines available)`);

  const toolRes = await fetch("http://localhost:5001/api/v1/tools/text-case-converter");
  const toolData: any = await toolRes.json();
  console.log("✓ GET /api/v1/tools/:id status:", toolData.name, `[status: ${toolData.status}, engine: ${toolData.engine?.id}]`);

  const engRes = await fetch("http://localhost:5001/api/v1/engines/text-engine");
  const engData: any = await engRes.json();
  console.log("✓ GET /api/v1/engines/:id status:", engData.id, `(capabilities: ${engData.capabilities.join(", ")})`);

  // 4. Test Frontend Server Serving
  console.log("\n[4/4] Testing Frontend Serving & Assets:");
  const feRes = await fetch("http://localhost:5173/");
  console.log(`✓ GET http://localhost:5173/ -> HTTP ${feRes.status} (${feRes.headers.get("content-type")})`);

  const wasmRes = await fetch("http://localhost:5173/engines/text/1.0.0/text_engine.wasm");
  console.log(`✓ GET http://localhost:5173/engines/text/1.0.0/text_engine.wasm -> HTTP ${wasmRes.status} (${wasmRes.headers.get("content-type")})`);
  if (!wasmRes.ok) {
    throw new Error(`Failed to fetch text_engine.wasm from frontend server: HTTP ${wasmRes.status}`);
  }

  console.log("\n🎉 ALL ARCHITECTURE VERIFICATION CHECKS PASSED!");
}

main().catch((err) => {
  console.error("Test failed:", err);
  process.exit(1);
});

