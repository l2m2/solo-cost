import { create } from "zustand";
import { call } from "@/lib/ipc";
import { useCompanyStore } from "./company";
import { useCategoriesStore } from "./categories";
import { useProjectsStore } from "./projects";
import { useCostsStore } from "./costs";
import { useTrashStore } from "./trash";
import { useFinancialStore } from "./financial";
import { useMembersStore } from "./members";
import { useClientsStore } from "./clients";
import { usePaymentsStore } from "./payments";
import { useTasksStore } from "./tasks";
import { useTimelogsStore } from "./timelogs";
import { useBackupStore } from "./backup";

type Status = "unknown" | "uninitialized" | "locked" | "unlocked" | "corrupted";

interface AuthState {
  status: Status;
  refresh: () => Promise<void>;
  setup: (password: string) => Promise<void>;
  unlock: (password: string) => Promise<void>;
  lock: () => Promise<void>;
}

export const useAuthStore = create<AuthState>((set, get) => ({
  status: "unknown",
  async refresh() {
    // If integrity check failed, do not overwrite the corrupted status.
    if (get().status === "corrupted") return;
    const initialized = await call<boolean>("is_initialized");
    set({ status: initialized ? "locked" : "uninitialized" });
  },
  async setup(password) {
    await call<void>("setup", { password });
    set({ status: "unlocked" });
  },
  async unlock(password) {
    try {
      await call<void>("unlock", { password });
      // fire-and-forget: try to snapshot right after unlock
      void useBackupStore.getState().maybeAutoBackup().catch(() => {});
      set({ status: "unlocked" });
    } catch (e: unknown) {
      const msg = String(e);
      if (msg.includes("integrity check failed")) {
        set({ status: "corrupted" });
      }
      throw e;
    }
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
    useMembersStore.getState().reset();
    useClientsStore.getState().reset();
    usePaymentsStore.getState().reset();
    useTasksStore.getState().reset();
    useTimelogsStore.getState().reset();
    useBackupStore.getState().reset();
    set({ status: "locked" });
  },
}));
