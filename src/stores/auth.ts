import { create } from "zustand";
import { call } from "@/lib/ipc";

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
    set({ status: "locked" });
  },
}));
