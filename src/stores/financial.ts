import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { ProjectFinancialSummary } from "@/types";

interface S {
  byProject: Record<number, ProjectFinancialSummary>;
  refresh: (projectId: number) => Promise<void>;
  reset: () => void;
}

export const useFinancialStore = create<S>((set, get) => ({
  byProject: {},
  async refresh(projectId) {
    try {
      const f = await call<ProjectFinancialSummary>("get_project_financial_summary", { id: projectId });
      set({ byProject: { ...get().byProject, [projectId]: f } });
    } catch {
      // ignore — non-fatal; overview will show "—"
    }
  },
  reset() {
    set({ byProject: {} });
  },
}));
