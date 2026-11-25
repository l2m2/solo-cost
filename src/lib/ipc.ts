import { invoke } from "@tauri-apps/api/core";

import type { Project, ProjectStatusEvent } from "@/types";

export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(cmd, args);
}

export function listProjectStatusEvents(id: number): Promise<ProjectStatusEvent[]> {
  return call("list_project_status_events", { id });
}

export function changeProjectStatus(id: number, status: string, target?: string, reason?: string): Promise<Project> {
  return call("set_project_status", { id, status, target, reason });
}
