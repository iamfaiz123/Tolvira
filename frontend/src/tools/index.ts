import React, { lazy } from "react";
import type { ToolDefinition } from "../types";

export interface ToolComponentProps {
  tool: ToolDefinition;
}

export const TOOL_COMPONENTS: Record<
  string,
  React.LazyExoticComponent<React.ComponentType<ToolComponentProps>>
> = {
  TextCaseConverter: lazy(() => import("./text/CaseConverterTool")),
  WordCounter: lazy(() => import("./text/WordCounterTool")),
  JsonFormatter: lazy(() => import("./json/JsonFormatterTool")),
  Base64EncoderDecoder: lazy(() => import("./encoding/Base64Tool")),
  UrlEncoderDecoder: lazy(() => import("./encoding/UrlEncoderTool")),
  ImageCropperTool: lazy(() => import("./image/ImageCropperTool")),
  EngineToolRunner: lazy(() => import("./engine/EngineToolRunner")),
};

export function getToolComponent(
  componentName?: string,
): React.LazyExoticComponent<React.ComponentType<ToolComponentProps>> | null {
  if (!componentName) return null;
  return TOOL_COMPONENTS[componentName] || null;
}

