import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { ModuleLaborStat } from "@/types";

interface S {
  byProject: Record<number, ModuleLaborStat[]>;
  refresh: (projectId: number) => Promise<void>;
}

export const useModuleStatsStore = create<S>((set, get) => ({
  byProject: {},
  async refresh(projectId) {
    const stats = await call<ModuleLaborStat[]>("get_module_labor_stats", { projectId });
    set({ byProject: { ...get().byProject, [projectId]: stats } });
  },
}));
