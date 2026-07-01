import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { CostCategory } from "@/types";

interface S {
  list: CostCategory[];
  loadedForCompany: number | null;
  loadFor: (companyId: number) => Promise<void>;
  create: (companyId: number, name: string) => Promise<void>;
  update: (id: number, name: string) => Promise<void>;
  remove: (id: number) => Promise<void>;
  reset: () => void;
}

export const useCategoriesStore = create<S>((set, get) => ({
  list: [],
  loadedForCompany: null,
  async loadFor(companyId) {
    // seed presets first (idempotent), then refresh
    const list = await call<CostCategory[]>("seed_preset_categories_if_empty", {
      companyId,
    });
    set({ list, loadedForCompany: companyId });
  },
  async create(companyId, name) {
    const c = await call<CostCategory>("create_category", {
      companyId,
      input: { name },
    });
    set({ list: [...get().list, c] });
  },
  async update(id, name) {
    const c = await call<CostCategory>("update_category", { id, input: { name } });
    set({ list: get().list.map((x) => (x.id === id ? c : x)) });
  },
  async remove(id) {
    await call<void>("delete_category", { id });
    set({ list: get().list.filter((x) => x.id !== id) });
  },
  reset() {
    set({ list: [], loadedForCompany: null });
  },
}));
