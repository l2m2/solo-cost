import { create } from "zustand";
import { call } from "@/lib/ipc";
import { useCompanyStore } from "./company";
import { useCategoriesStore } from "./categories";
import { useProjectsStore } from "./projects";
import { useCostsStore } from "./costs";
import { useTrashStore } from "./trash";
import { useFinancialStore } from "./financial";

type Status = "unknown" | "uninitialized" | "locked" | "unlocked";

interface AuthState {
  status: Status;
  refresh: () => Promise<void>;
  setup: (password: string) => Promise<void>;
  unlock: (password: string) => Promise<void>;
  lock: () => Promise<void>;
}

export const useAuthStore = create<AuthState>((set) => ({
  status: "unknown",
  async refresh() {
    const initialized = await call<boolean>("is_initialized");
    set({ status: initialized ? "locked" : "uninitialized" });
  },
  async setup(password) {
    await call<void>("setup", { password });
    set({ status: "unlocked" });
  },
  async unlock(password) {
    await call<void>("unlock", { password });
    set({ status: "unlocked" });
  },
  async lock() {
    await call<void>("lock");
    // reset all entity stores so a re-unlock pulls fresh data
    useCompanyStore.getState().reset();
    useCategoriesStore.getState().reset();
    useProjectsStore.getState().reset();
    useCostsStore.getState().reset();
    useTrashStore.getState().reset();
    useFinancialStore.getState().reset();
    set({ status: "locked" });
  },
}));
