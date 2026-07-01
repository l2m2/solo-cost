import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { CostEntry, CostEntryInput, ProjectCostSummary } from "@/types";

interface S {
  entriesByProject: Record<number, CostEntry[]>;
  summaryByProject: Record<number, ProjectCostSummary>;
  loadFor: (projectId: number) => Promise<void>;
  create: (projectId: number, input: CostEntryInput) => Promise<void>;
  update: (id: number, input: CostEntryInput, projectId: number) => Promise<void>;
  remove: (id: number, projectId: number) => Promise<void>;
  reset: () => void;
}

async function refresh(projectId: number) {
  const [entries, summary] = await Promise.all([
    call<CostEntry[]>("list_cost_entries", { projectId }),
    call<ProjectCostSummary>("get_project_cost_summary", { projectId }),
  ]);
  return { entries, summary };
}

export const useCostsStore = create<S>((set, get) => ({
  entriesByProject: {},
  summaryByProject: {},
  async loadFor(projectId) {
    const { entries, summary } = await refresh(projectId);
    set({
      entriesByProject: { ...get().entriesByProject, [projectId]: entries },
      summaryByProject: { ...get().summaryByProject, [projectId]: summary },
    });
  },
  async create(projectId, input) {
    await call<CostEntry>("create_cost_entry", { projectId, input });
    await get().loadFor(projectId);
  },
  async update(id, input, projectId) {
    await call<CostEntry>("update_cost_entry", { id, input });
    await get().loadFor(projectId);
  },
  async remove(id, projectId) {
    await call<void>("delete_cost_entry", { id });
    await get().loadFor(projectId);
  },
  reset() {
    set({ entriesByProject: {}, summaryByProject: {} });
  },
}));
