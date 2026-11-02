import { useEffect, useState } from "react";
import initialTracker from "../../../tracker.json";

export type TestStatus = "untested" | "passed" | "fixed" | "issue" | "queued";

export interface ToolTrackingInfo {
  status: TestStatus;
  testedAt?: string;
  notes?: string;
}

export type TrackerState = Record<string, ToolTrackingInfo>;

const STORAGE_KEY = "dev_toolbox_tracker_v1";
const EVENT_NAME = "dev_toolbox_tracker_updated";

function getInitialState(): TrackerState {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved) {
      return JSON.parse(saved);
    }
  } catch (e) {
    console.error("Failed to read tracker from localStorage", e);
  }

  // Fallback to tracker.json seed
  const state: TrackerState = {};
  if (initialTracker && initialTracker.tools) {
    for (const [slug, data] of Object.entries(initialTracker.tools as Record<string, any>)) {
      state[slug] = {
        status: data.test_status || "untested",
        testedAt: data.tested_at || undefined,
        notes: data.notes || "",
      };
    }
  }
  return state;
}

let currentState: TrackerState = getInitialState();

export function getToolStatus(slug: string): ToolTrackingInfo {
  return currentState[slug] || { status: "untested", notes: "" };
}

export function updateToolStatus(slug: string, status: TestStatus, notes?: string): void {
  const prev = currentState[slug] || { status: "untested", notes: "" };
  const updated: ToolTrackingInfo = {
    ...prev,
    status,
    testedAt: new Date().toISOString(),
    notes: notes !== undefined ? notes : prev.notes,
  };

  currentState = {
    ...currentState,
    [slug]: updated,
  };

  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(currentState));
  } catch (e) {
    console.error("Failed to save tracker to localStorage", e);
  }

  window.dispatchEvent(new CustomEvent(EVENT_NAME, { detail: { slug, updated } }));
}

export function resetTrackerState(): void {
  localStorage.removeItem(STORAGE_KEY);
  currentState = getInitialState();
  window.dispatchEvent(new CustomEvent(EVENT_NAME));
}

export function exportTrackerJson(): string {
  return JSON.stringify(currentState, null, 2);
}

export function importTrackerJson(jsonString: string): boolean {
  try {
    const parsed = JSON.parse(jsonString);
    currentState = { ...currentState, ...parsed };
    localStorage.setItem(STORAGE_KEY, JSON.stringify(currentState));
    window.dispatchEvent(new CustomEvent(EVENT_NAME));
    return true;
  } catch (e) {
    console.error("Failed to import tracker JSON", e);
    return false;
  }
}

/** React hook for subscribing to tracker updates. */
export function useTracker() {
  const [tracker, setTracker] = useState<TrackerState>(currentState);

  useEffect(() => {
    const handler = () => {
      setTracker({ ...currentState });
    };
    window.addEventListener(EVENT_NAME, handler);
    return () => window.removeEventListener(EVENT_NAME, handler);
  }, []);

  return {
    tracker,
    updateStatus: updateToolStatus,
    getStatus: (slug: string) => tracker[slug] || { status: "untested", notes: "" },
    resetTracker: resetTrackerState,
    exportJson: exportTrackerJson,
    importJson: importTrackerJson,
  };
}

