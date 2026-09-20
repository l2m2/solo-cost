import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { McpStatus } from "@/types";

interface McpState {
  status: McpStatus | null;
  loading: boolean;
  error: string | null;
  loadStatus: () => Promise<void>;
}

export const useMcpStore = create<McpState>((set) => ({
  status: null,
  loading: false,
  error: null,
  async loadStatus() {
    set({ loading: true, error: null });
    try {
      const status = await call<McpStatus>("get_mcp_status");
      set({ status, loading: false });
    } catch (error) {
      set({ loading: false, error: String(error) });
    }
  },
}));

