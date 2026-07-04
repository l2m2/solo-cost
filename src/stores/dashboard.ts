import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { DashboardSummary } from "@/types";

interface S {
  data: DashboardSummary | null;
  loadedForCompany: number | null;
  loadFor: (companyId: number) => Promise<void>;
  reset: () => void;
}

export const useDashboardStore = create<S>((set) => ({
  data: null,
  loadedForCompany: null,
  async loadFor(companyId) {
    try {
      const d = await call<DashboardSummary>("get_dashboard", { companyId });
      set({ data: d, loadedForCompany: companyId });
    } catch {
      // non-fatal; page shows loading/empty
      set({ data: null, loadedForCompany: companyId });
    }
  },
  reset() {
    set({ data: null, loadedForCompany: null });
  },
}));
