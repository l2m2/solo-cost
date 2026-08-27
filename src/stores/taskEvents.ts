import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { TaskEvent } from "@/types";

interface S {
  byTask: Record<number, TaskEvent[]>;
  loadFor: (taskId: number) => Promise<void>;
  createNote: (taskId: number, body: string, occurredAt?: string | null) => Promise<void>;
  updateNote: (id: number, body: string, taskId: number) => Promise<void>;
  deleteNote: (id: number, taskId: number) => Promise<void>;
  reset: () => void;
}

export const useTaskEventsStore = create<S>((set, get) => ({
  byTask: {},
  async loadFor(taskId) {
    const list = await call<TaskEvent[]>("list_task_events", { taskId });
    set({ byTask: { ...get().byTask, [taskId]: list } });
  },
  async createNote(taskId, body, occurredAt = null) {
    await call<TaskEvent>("create_task_note", { taskId, body, occurredAt });
    await get().loadFor(taskId);
  },
  async updateNote(id, body, taskId) {
    await call<TaskEvent>("update_task_note", { id, body });
    await get().loadFor(taskId);
  },
  async deleteNote(id, taskId) {
    await call<void>("delete_task_note", { id });
    await get().loadFor(taskId);
  },
  reset() {
    set({ byTask: {} });
  },
}));
