import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { Task, TaskInput } from "@/types";
import { useFinancialStore } from "./financial";
import { useModuleStatsStore } from "./moduleStats";

interface S {
  byProject: Record<number, Task[]>;
  statusFilter: string | null;
  loadFor: (projectId: number, statusFilter?: string | null) => Promise<void>;
  create: (projectId: number, input: TaskInput) => Promise<Task>;
  update: (id: number, input: TaskInput, projectId: number) => Promise<Task>;
  setStatus: (id: number, status: string, projectId: number) => Promise<void>;
  softDelete: (id: number, projectId: number) => Promise<void>;
  reset: () => void;
}

export const useTasksStore = create<S>((set, get) => ({
  byProject: {},
  statusFilter: null,
  async loadFor(projectId, statusFilter = null) {
    const list = await call<Task[]>("list_tasks", { projectId, status: statusFilter });
    set({ byProject: { ...get().byProject, [projectId]: list }, statusFilter });
  },
  async create(projectId, input) {
    const t = await call<Task>("create_task", { projectId, input });
    await get().loadFor(projectId, get().statusFilter);
    await useModuleStatsStore.getState().refresh(projectId);
    return t;
  },
  async update(id, input, projectId) {
    const t = await call<Task>("update_task", { id, input });
    await get().loadFor(projectId, get().statusFilter);
    await useModuleStatsStore.getState().refresh(projectId);
    return t;
  },
  async setStatus(id, status, projectId) {
    await call<Task>("set_task_status", { id, status });
    await get().loadFor(projectId, get().statusFilter);
  },
  async softDelete(id, projectId) {
    // Task delete cascades to time_logs, which affects labor cost
    await call<void>("delete_task", { id });
    try {
      await get().loadFor(projectId, get().statusFilter);
    } finally {
      await useFinancialStore.getState().refresh(projectId);
      await useModuleStatsStore.getState().refresh(projectId);
    }
  },
  reset() {
    set({ byProject: {}, statusFilter: null });
  },
}));
