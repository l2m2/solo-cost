import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { TimeLog, TimeLogInput, TimeLogUpdateInput } from "@/types";
import { useFinancialStore } from "./financial";

interface S {
  byTask: Record<number, TimeLog[]>;
  loadFor: (taskId: number) => Promise<void>;
  // projectId is required for financial store refresh after mutations
  create: (input: TimeLogInput, projectId: number) => Promise<void>;
  update: (id: number, input: TimeLogUpdateInput, taskId: number, projectId: number) => Promise<void>;
  softDelete: (id: number, taskId: number, projectId: number) => Promise<void>;
  reset: () => void;
}

export const useTimelogsStore = create<S>((set, get) => ({
  byTask: {},
  async loadFor(taskId) {
    const list = await call<TimeLog[]>("list_time_logs_by_task", { taskId });
    set({ byTask: { ...get().byTask, [taskId]: list } });
  },
  async create(input, projectId) {
    await call<TimeLog>("create_time_log", { input });
    await get().loadFor(input.task_id);
    await useFinancialStore.getState().refresh(projectId);
  },
  async update(id, input, taskId, projectId) {
    await call<TimeLog>("update_time_log", { id, input });
    await get().loadFor(taskId);
    await useFinancialStore.getState().refresh(projectId);
  },
  async softDelete(id, taskId, projectId) {
    await call<void>("delete_time_log", { id });
    try {
      await get().loadFor(taskId);
    } finally {
      await useFinancialStore.getState().refresh(projectId);
    }
  },
  reset() {
    set({ byTask: {} });
  },
}));
