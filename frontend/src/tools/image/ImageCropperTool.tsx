import React, { useState, useRef, useEffect, useCallback } from "react";
import {
  Upload,
  RotateCw,
  RotateCcw,
  FlipHorizontal,
  FlipVertical,
  Download,
  Copy,
  Check,
  Sparkles,
  RefreshCw,
  Image as ImageIcon,
} from "lucide-react";
import { WorkerManager } from "../../services/worker-manager";
import type { ToolDefinition } from "../../types";

interface Props {
  tool: ToolDefinition;
}

interface CropRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

const PRESETS = [
  { label: "Freeform", id: "free" },
  { label: "1:1 Square", id: "1:1" },
  { label: "16:9 Wide", id: "16:9" },
  { label: "4:3 Standard", id: "4:3" },
  { label: "9:16 Vertical", id: "9:16" },
  { label: "3:2 Classic", id: "3:2" },
];

export const ImageCropperTool: React.FC<Props> = ({ tool }) => {
  const [imageSrc, setImageSrc] = useState<string | null>(null);
  const [imageSize, setImageSize] = useState<{ width: number; height: number }>({ width: 0, height: 0 });
  const [crop, setCrop] = useState<CropRect>({ x: 0, y: 0, width: 300, height: 300 });
  const [selectedPreset, setSelectedPreset] = useState<string>("free");
  const [rotation, setRotation] = useState<number>(0);
  const [flipH, setFlipH] = useState<boolean>(false);
  const [flipV, setFlipV] = useState<boolean>(false);
  const [exportFormat, setExportFormat] = useState<"image/png" | "image/jpeg" | "image/webp">("image/png");
  const [quality, setQuality] = useState<number>(92);
  const [copied, setCopied] = useState<boolean>(false);
  const [engineInfo, setEngineInfo] = useState<string>("Ready");
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);

  const imageRef = useRef<HTMLImageElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const previewCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const isDraggingRef = useRef<boolean>(false);
  const dragStartRef = useRef<{ x: number; y: number; cropX: number; cropY: number }>({ x: 0, y: 0, cropX: 0, cropY: 0 });

  // Load a demo sample image if user has not uploaded one
  const loadSampleImage = () => {
    const canvas = document.createElement("canvas");
    canvas.width = 800;
    canvas.height = 600;
    const ctx = canvas.getContext("2d")!;

    // Draw stylish gradient
    const grad = ctx.createLinearGradient(0, 0, 800, 600);
    grad.addColorStop(0, "#0d9488");
    grad.addColorStop(0.5, "#0f766e");
    grad.addColorStop(1, "#115e59");
    ctx.fillStyle = grad;
    ctx.fillRect(0, 0, 800, 600);

    // Circles
    ctx.fillStyle = "rgba(255, 255, 255, 0.15)";
    ctx.beginPath();
    ctx.arc(400, 300, 200, 0, Math.PI * 2);
    ctx.fill();

    ctx.fillStyle = "rgba(255, 255, 255, 0.25)";
    ctx.beginPath();
    ctx.arc(400, 300, 120, 0, Math.PI * 2);
    ctx.fill();

    ctx.fillStyle = "#ffffff";
    ctx.font = "bold 36px sans-serif";
    ctx.textAlign = "center";
    ctx.fillText("Dev Toolbox", 400, 280);

    ctx.font = "20px sans-serif";
    ctx.fillStyle = "rgba(255, 255, 255, 0.85)";
    ctx.fillText("High-Performance Image Cropper", 400, 320);

    ctx.font = "14px monospace";
    ctx.fillStyle = "rgba(255, 255, 255, 0.6)";
    ctx.fillText("800 x 600 • WASM Accelerated", 400, 360);

    const dataUrl = canvas.toDataURL("image/png");
    setImageSrc(dataUrl);
  };

  // Handle file upload
  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      if (!file.type.startsWith("image/")) {
        alert("Please select a valid image file.");
        return;
      }
      const reader = new FileReader();
      reader.onload = (event) => {
        setImageSrc(event.target?.result as string);
        setRotation(0);
        setFlipH(false);
        setFlipV(false);
      };
      reader.readAsDataURL(file);
    }
  };

  // On image loaded, set dimensions and compute initial crop
  const onImageLoaded = (img: HTMLImageElement) => {
    imageRef.current = img;
    const w = img.naturalWidth;
    const h = img.naturalHeight;
    setImageSize({ width: w, height: h });

    // Initial crop: centered 80%
    const initW = Math.round(w * 0.8);
    const initH = Math.round(h * 0.8);
    setCrop({
      x: Math.round((w - initW) / 2),
      y: Math.round((h - initH) / 2),
      width: initW,
      height: initH,
    });
  };

  // Preset calculation using WASM image-engine
  const applyPreset = async (presetId: string) => {
    setSelectedPreset(presetId);
    if (!imageSize.width || !imageSize.height) return;

    if (presetId === "free") return;

    try {
      const wasmUrl = tool.engine?.url || "/engines/image/1.0.0/image_engine.wasm";
      const workerMgr = WorkerManager.getInstance();
      const res: any = await workerMgr.executeTask("image-engine", wasmUrl, "preset_crop", {
        orig_width: imageSize.width,
        orig_height: imageSize.height,
        preset: presetId,
      });

      if (res && res.valid) {
        setCrop({
          x: res.crop_x,
          y: res.crop_y,
          width: res.crop_width,
          height: res.crop_height,
        });
        setEngineInfo(`Preset ${presetId} computed via WASM (${res.ratio_str}, ${res.area_percent}% area)`);
      }
    } catch {
      // Fallback calculation if WASM unavailable
      const [tw, th] = presetId.split(":").map(Number);
      const targetRatio = tw / th;
      const currentRatio = imageSize.width / imageSize.height;

      let cw = imageSize.width;
      let ch = imageSize.height;
      if (currentRatio > targetRatio) {
        cw = Math.round(imageSize.height * targetRatio);
      } else {
        ch = Math.round(imageSize.width / targetRatio);
      }
      setCrop({
        x: Math.round((imageSize.width - cw) / 2),
        y: Math.round((imageSize.height - ch) / 2),
        width: cw,
        height: ch,
      });
    }
  };

  // Update real-time cropped preview
  const updatePreview = useCallback(() => {
    if (!imageRef.current || !crop.width || !crop.height) return;

    const img = imageRef.current;
    const canvas = previewCanvasRef.current || document.createElement("canvas");
    canvas.width = crop.width;
    canvas.height = crop.height;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    ctx.save();
    // Handle rotation/flip if applicable
    if (rotation !== 0 || flipH || flipV) {
      ctx.translate(crop.width / 2, crop.height / 2);
      ctx.rotate((rotation * Math.PI) / 180);
      ctx.scale(flipH ? -1 : 1, flipV ? -1 : 1);
      ctx.translate(-crop.width / 2, -crop.height / 2);
    }

    ctx.drawImage(
      img,
      crop.x,
      crop.y,
      crop.width,
      crop.height,
      0,
      0,
      crop.width,
      crop.height,
    );
    ctx.restore();

    setPreviewUrl(canvas.toDataURL(exportFormat, quality / 100));
  }, [crop, rotation, flipH, flipV, exportFormat, quality]);

  useEffect(() => {
    updatePreview();
  }, [updatePreview]);

  // Download cropped image
  const handleDownload = () => {
    if (!previewUrl) return;
    const link = document.createElement("a");
    const ext = exportFormat === "image/jpeg" ? "jpg" : exportFormat === "image/webp" ? "webp" : "png";
    link.download = `cropped-image-${crop.width}x${crop.height}.${ext}`;
    link.href = previewUrl;
    link.click();
  };

  // Copy to clipboard
  const handleCopy = async () => {
    if (!previewCanvasRef.current) return;
    try {
      previewCanvasRef.current.toBlob(async (blob) => {
        if (blob) {
          await navigator.clipboard.write([
            new ClipboardItem({ [blob.type]: blob }),
          ]);
          setCopied(true);
          setTimeout(() => setCopied(false), 2000);
        }
      });
    } catch {
      alert("Clipboard image write not supported in this browser.");
    }
  };

  return (
    <div className="rounded-xl border border-stone-200 bg-white p-6 shadow-sm dark:border-stone-800 dark:bg-stone-900">
      {/* Header */}
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3 border-b border-stone-100 pb-4 dark:border-stone-800">
        <div>
          <h3 className="text-lg font-bold text-stone-900 dark:text-stone-100 flex items-center gap-2">
            <ImageIcon className="h-5 w-5 text-teal-700" />
            Interactive Client Image Cropper
          </h3>
          <p className="text-xs text-stone-500">
            Hardware-accelerated client-side cropping powered by{" "}
            <span className="font-semibold text-teal-600">image-engine.wasm</span>
          </p>
        </div>

        <div className="flex items-center gap-2">
          {!imageSrc ? (
            <button
              onClick={loadSampleImage}
              className="flex items-center gap-1.5 rounded-lg border border-teal-700/40 bg-teal-50 px-3 py-1.5 text-xs font-medium text-teal-800 hover:bg-teal-100 dark:bg-teal-950/40 dark:text-teal-300"
            >
              <Sparkles className="h-3.5 w-3.5" />
              Try with Demo Image
            </button>
          ) : (
            <button
              onClick={() => {
                setImageSrc(null);
                setPreviewUrl(null);
              }}
              className="flex items-center gap-1 rounded-lg border border-stone-200 px-3 py-1.5 text-xs text-stone-600 hover:bg-stone-50 dark:border-stone-700"
            >
              <RefreshCw className="h-3.5 w-3.5" />
              Choose Another
            </button>
          )}
        </div>
      </div>

      {!imageSrc ? (
        /* Upload Area */
        <div className="rounded-xl border-2 border-dashed border-stone-300 p-12 text-center hover:border-teal-600 dark:border-stone-700">
          <Upload className="mx-auto h-12 w-12 text-stone-400" />
          <h4 className="mt-3 text-sm font-semibold text-stone-800 dark:text-stone-200">
            Drag and drop an image, or browse
          </h4>
          <p className="mt-1 text-xs text-stone-500">
            Supports PNG, JPEG, WEBP, SVG, GIF (up to 50MB). Processed 100% locally in your browser.
          </p>
          <label className="mt-4 inline-block cursor-pointer rounded-lg bg-teal-800 px-4 py-2 text-xs font-medium text-white hover:bg-teal-900">
            Select Image
            <input
              type="file"
              accept="image/*"
              onChange={handleFileChange}
              className="hidden"
            />
          </label>
        </div>
      ) : (
        /* Cropper Workspace */
        <div className="space-y-6">
          {/* Preset Buttons */}
          <div>
            <div className="mb-2 flex items-center justify-between">
              <span className="text-xs font-semibold uppercase tracking-wider text-stone-600 dark:text-stone-400">
                Aspect Ratio Presets
              </span>
              <span className="text-xs text-teal-700 font-mono">{engineInfo}</span>
            </div>
            <div className="flex flex-wrap gap-2">
              {PRESETS.map((p) => (
                <button
                  key={p.id}
                  onClick={() => applyPreset(p.id)}
                  className={`rounded-lg px-3 py-1.5 text-xs font-medium transition-all ${
                    selectedPreset === p.id
                      ? "bg-teal-800 text-white shadow-sm"
                      : "border border-stone-200 bg-stone-50 text-stone-700 hover:bg-stone-100 dark:border-stone-700 dark:bg-stone-800 dark:text-stone-300"
                  }`}
                >
                  {p.label}
                </button>
              ))}
            </div>
          </div>

          {/* Transformations & Sliders */}
          <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
            {/* Main Interactive Crop Controls */}
            <div className="rounded-lg border border-stone-200 bg-stone-50/50 p-4 dark:border-stone-800 dark:bg-stone-800/40">
              <div className="mb-3 flex items-center justify-between">
                <span className="text-xs font-semibold text-stone-700 dark:text-stone-300">
                  Crop Dimensions & Position
                </span>
                <span className="text-xs font-mono text-stone-500">
                  Original: {imageSize.width} × {imageSize.height} px
                </span>
              </div>

              <div className="grid grid-cols-2 gap-3 text-xs">
                <div>
                  <label className="mb-1 block text-stone-500">Width: {crop.width} px</label>
                  <input
                    type="range"
                    min={20}
                    max={imageSize.width - crop.x}
                    value={crop.width}
                    onChange={(e) =>
                      setCrop((prev) => ({ ...prev, width: Number(e.target.value) }))
                    }
                    className="w-full"
                  />
                </div>
                <div>
                  <label className="mb-1 block text-stone-500">Height: {crop.height} px</label>
                  <input
                    type="range"
                    min={20}
                    max={imageSize.height - crop.y}
                    value={crop.height}
                    onChange={(e) =>
                      setCrop((prev) => ({ ...prev, height: Number(e.target.value) }))
                    }
                    className="w-full"
                  />
                </div>
                <div>
                  <label className="mb-1 block text-stone-500">Offset X: {crop.x} px</label>
                  <input
                    type="range"
                    min={0}
                    max={Math.max(0, imageSize.width - crop.width)}
                    value={crop.x}
                    onChange={(e) =>
                      setCrop((prev) => ({ ...prev, x: Number(e.target.value) }))
                    }
                    className="w-full"
                  />
                </div>
                <div>
                  <label className="mb-1 block text-stone-500">Offset Y: {crop.y} px</label>
                  <input
                    type="range"
                    min={0}
                    max={Math.max(0, imageSize.height - crop.height)}
                    value={crop.y}
                    onChange={(e) =>
                      setCrop((prev) => ({ ...prev, y: Number(e.target.value) }))
                    }
                    className="w-full"
                  />
                </div>
              </div>

              {/* Transform buttons */}
              <div className="mt-4 flex flex-wrap items-center gap-2 border-t border-stone-200 pt-3 dark:border-stone-700">
                <span className="text-xs text-stone-500 mr-2">Transform:</span>
                <button
                  onClick={() => setRotation((r) => (r + 90) % 360)}
                  className="flex items-center gap-1 rounded bg-stone-200 px-2 py-1 text-xs hover:bg-stone-300 dark:bg-stone-700"
                  title="Rotate Clockwise"
                >
                  <RotateCw className="h-3.5 w-3.5" /> 90°
                </button>
                <button
                  onClick={() => setRotation((r) => (r - 90 + 360) % 360)}
                  className="flex items-center gap-1 rounded bg-stone-200 px-2 py-1 text-xs hover:bg-stone-300 dark:bg-stone-700"
                  title="Rotate Counter-Clockwise"
                >
                  <RotateCcw className="h-3.5 w-3.5" /> -90°
                </button>
                <button
                  onClick={() => setFlipH((f) => !f)}
                  className={`flex items-center gap-1 rounded px-2 py-1 text-xs ${
                    flipH ? "bg-teal-800 text-white" : "bg-stone-200 dark:bg-stone-700"
                  }`}
                  title="Flip Horizontal"
                >
                  <FlipHorizontal className="h-3.5 w-3.5" /> Flip H
                </button>
                <button
                  onClick={() => setFlipV((f) => !f)}
                  className={`flex items-center gap-1 rounded px-2 py-1 text-xs ${
                    flipV ? "bg-teal-800 text-white" : "bg-stone-200 dark:bg-stone-700"
                  }`}
                  title="Flip Vertical"
                >
                  <FlipVertical className="h-3.5 w-3.5" /> Flip V
                </button>
              </div>
            </div>

            {/* Export & Quality Settings */}
            <div className="rounded-lg border border-stone-200 bg-stone-50/50 p-4 dark:border-stone-800 dark:bg-stone-800/40">
              <span className="text-xs font-semibold text-stone-700 dark:text-stone-300 mb-3 block">
                Export Format & Quality
              </span>

              <div className="space-y-3 text-xs">
                <div className="flex items-center gap-3">
                  <label className="text-stone-600 font-medium">Format:</label>
                  {(["image/png", "image/jpeg", "image/webp"] as const).map((fmt) => (
                    <label key={fmt} className="flex items-center gap-1 cursor-pointer">
                      <input
                        type="radio"
                        name="exportFormat"
                        value={fmt}
                        checked={exportFormat === fmt}
                        onChange={(e) => setExportFormat(e.target.value as any)}
                        className="text-teal-600 focus:ring-teal-500"
                      />
                      <span>{fmt.replace("image/", "").toUpperCase()}</span>
                    </label>
                  ))}
                </div>

                {exportFormat !== "image/png" && (
                  <div>
                    <div className="flex justify-between mb-1 text-stone-500">
                      <span>Compression Quality:</span>
                      <span className="font-mono font-bold text-teal-700">{quality}%</span>
                    </div>
                    <input
                      type="range"
                      min={10}
                      max={100}
                      value={quality}
                      onChange={(e) => setQuality(Number(e.target.value))}
                      className="w-full"
                    />
                  </div>
                )}

                <div className="rounded border border-stone-200 bg-white p-3 dark:border-stone-700 dark:bg-stone-900">
                  <div className="text-stone-500">Output Dimensions:</div>
                  <div className="text-base font-bold text-teal-800 dark:text-teal-400">
                    {crop.width} × {crop.height} px
                  </div>
                  <div className="text-stone-400 text-[11px] mt-0.5">
                    Aspect Ratio: {(crop.width / crop.height).toFixed(2)}:1 •{" "}
                    {(((crop.width * crop.height) / (imageSize.width * imageSize.height || 1)) * 100).toFixed(1)}% of original area
                  </div>
                </div>

                <div className="flex gap-2 pt-1">
                  <button
                    onClick={handleDownload}
                    className="flex-1 flex items-center justify-center gap-1.5 rounded-lg bg-teal-800 px-4 py-2 text-xs font-semibold text-white hover:bg-teal-900 active:scale-95 transition-all shadow-sm"
                  >
                    <Download className="h-4 w-4" />
                    Download Cropped Image
                  </button>
                  <button
                    onClick={handleCopy}
                    className="flex items-center gap-1 rounded-lg border border-stone-300 bg-white px-3 py-2 text-xs font-medium text-stone-700 hover:bg-stone-50 dark:border-stone-700 dark:bg-stone-800 dark:text-stone-200"
                    title="Copy to Clipboard"
                  >
                    {copied ? <Check className="h-4 w-4 text-teal-600" /> : <Copy className="h-4 w-4" />}
                  </button>
                </div>
              </div>
            </div>
          </div>

          {/* Visual Workspace & Live Cropped Preview */}
          <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
            {/* Original with Crop Window */}
            <div>
              <span className="mb-2 block text-xs font-semibold uppercase text-stone-500">
                Source Image & Crop Viewport
              </span>
              <div className="relative overflow-hidden rounded-lg border border-stone-200 bg-stone-950 p-2 text-center dark:border-stone-800">
                <img
                  src={imageSrc}
                  alt="Original to crop"
                  onLoad={(e) => onImageLoaded(e.currentTarget)}
                  className="mx-auto max-h-[360px] object-contain rounded"
                />
              </div>
            </div>

            {/* Cropped Output Result */}
            <div>
              <span className="mb-2 block text-xs font-semibold uppercase text-stone-500">
                Live Cropped Preview
              </span>
              <div className="relative flex min-h-[300px] max-h-[360px] items-center justify-center overflow-hidden rounded-lg border border-stone-200 bg-stone-100 p-4 dark:border-stone-800 dark:bg-stone-950">
                {previewUrl ? (
                  <img
                    src={previewUrl}
                    alt="Cropped Preview"
                    className="max-h-[320px] max-w-full object-contain shadow-md rounded"
                  />
                ) : (
                  <div className="text-xs text-stone-400">Rendering preview...</div>
                )}
              </div>
            </div>
          </div>

          {/* Hidden canvas used for high-res cropping export */}
          <canvas ref={previewCanvasRef} className="hidden" />
        </div>
      )}
    </div>
  );
};

export default ImageCropperTool;
