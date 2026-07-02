import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { Module, ModuleInput } from "@/types";

interface S {
  byProject: Record<number, Module[]>;
  loadedForProject: Record<number, boolean>;
  loadFor: (projectId: number) => Promise<void>;
  create: (projectId: number, input: ModuleInput) => Promise<Module>;
  update: (id: number, input: ModuleInput, projectId: number) => Promise<Module>;
  moveUp: (id: number, projectId: number) => Promise<void>;
  moveDown: (id: number, projectId: number) => Promise<void>;
  softDelete: (id: number, projectId: number) => Promise<void>;
}

export const useModulesStore = create<S>((set, get) => ({
  byProject: {},
  loadedForProject: {},
  async loadFor(projectId) {
    const list = await call<Module[]>("list_modules", { projectId });
    set({
      byProject: { ...get().byProject, [projectId]: list },
      loadedForProject: { ...get().loadedForProject, [projectId]: true },
    });
  },
  async create(projectId, input) {
    const m = await call<Module>("create_module", { projectId, input });
    await get().loadFor(projectId);
    return m;
  },
  async update(id, input, projectId) {
    const m = await call<Module>("update_module", { id, input });
    await get().loadFor(projectId);
    return m;
  },
  async moveUp(id, projectId) {
    const list = get().byProject[projectId] ?? [];
    const idx = list.findIndex((m) => m.id === id);
    if (idx <= 0) return;
    const cur = list[idx];
    const prev = list[idx - 1];
    await call<Module>("update_module", {
      id: cur.id,
      input: { name: cur.name, sort_order: prev.sort_order },
    });
    await call<Module>("update_module", {
      id: prev.id,
      input: { name: prev.name, sort_order: cur.sort_order },
    });
    await get().loadFor(projectId);
  },
  async moveDown(id, projectId) {
    const list = get().byProject[projectId] ?? [];
    const idx = list.findIndex((m) => m.id === id);
    if (idx < 0 || idx >= list.length - 1) return;
    const cur = list[idx];
    const next = list[idx + 1];
    await call<Module>("update_module", {
      id: cur.id,
      input: { name: cur.name, sort_order: next.sort_order },
    });
    await call<Module>("update_module", {
      id: next.id,
      input: { name: next.name, sort_order: cur.sort_order },
    });
    await get().loadFor(projectId);
  },
  async softDelete(id, projectId) {
    await call<void>("delete_module", { id });
    await get().loadFor(projectId);
  },
}));
