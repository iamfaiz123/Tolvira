import rawSchemas from "../../../shared/data/engine-schemas.json";

/**
 * Form schema of every engine action, generated from the Rust action enums by
 * `scripts/generate-engine-schemas.py`. The generic tool UI renders real
 * inputs (file pickers, numbers, selects, …) from these definitions.
 */
export type FieldKind =
  | "text"
  | "textarea"
  | "number"
  | "integer"
  | "boolean"
  | "file"
  | "files"
  | "select"
  | "color"
  | "tags"
  | "numbers"
  | "matrix"
  | "json"
  | "keyvalues"
  | "group"
  | "list"
  | "variant";

export interface FieldOption {
  value: string;
  label: string;
  doc?: string;
  /** Fields of an internally tagged enum variant. */
  fields?: FieldSchema[];
}

export interface ListItemSchema {
  kind: "group" | "variant";
  fields?: FieldSchema[];
  tag?: string;
  variants?: FieldOption[];
}

export interface FieldSchema {
  name: string;
  label: string;
  doc: string;
  required: boolean;
  kind: FieldKind;
  default?: unknown;
  /** Value filled in automatically by the UI (`entropy`, `now_iso`, …). */
  auto?: string;
  accept?: string;
  options?: FieldOption[];
  allowCustom?: boolean;
  suggestions?: string[];
  min?: number;
  max?: number;
  slider?: boolean;
  unit?: string;
  fixed?: number;
  maxLength?: number;
  placeholder?: string;
  fields?: FieldSchema[];
  collapsible?: boolean;
  item?: ListItemSchema;
  tag?: string;
  variants?: FieldOption[];
  valueKind?: "number" | "text" | "any";
  array?: boolean;
}

export interface ActionSchema {
  doc: string;
  fields: FieldSchema[];
}

export interface EngineSchema {
  crate: string;
  doc: string;
  actions: Record<string, ActionSchema>;
}

const SCHEMAS = (rawSchemas as { engines: Record<string, EngineSchema> }).engines;

export function getEngineSchema(engineId: string): EngineSchema | undefined {
  return SCHEMAS[engineId];
}

export function getActionSchema(engineId: string, action: string): ActionSchema | undefined {
  return SCHEMAS[engineId]?.actions[action];
}

export function listActions(engineId: string): string[] {
  return Object.keys(SCHEMAS[engineId]?.actions ?? {});
}
