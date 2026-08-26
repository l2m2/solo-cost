import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { SearchHit } from "@/types";

type SearchState = {
  // Lifted out of CommandPalette so the Header trigger can open it too.
  open: boolean;
  setOpen: (v: boolean) => void;
  hits: SearchHit[];
  loading: boolean;
  run: (companyId: number, query: string) => Promise<void>;
  clear: () => void;
};

// Monotonic request id: a slow response must never overwrite a newer one.
let seq = 0;

export const useSearchStore = create<SearchState>((set) => ({
  open: false,
  setOpen: (v) => set({ open: v }),
  hits: [],
  loading: false,

  async run(companyId, query) {
    const q = query.trim();
    if (!q) {
      seq += 1; // invalidate any in-flight request
      set({ hits: [], loading: false });
      return;
    }
    const mine = ++seq;
    set({ loading: true });
    try {
      const hits = await call<SearchHit[]>("search", { companyId, query: q });
      if (mine === seq) set({ hits, loading: false });
    } catch {
      // A failed search should quietly show nothing rather than toast at the
      // user on every keystroke.
      if (mine === seq) set({ hits: [], loading: false });
    }
  },

  clear() {
    seq += 1;
    set({ hits: [], loading: false });
  },
}));
