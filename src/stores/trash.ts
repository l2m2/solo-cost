import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { TrashItem } from "@/types";

interface S {
  items: TrashItem[];
  loadedForCompany: number | null;
  loadFor: (companyId: number) => Promise<void>;
  restore: (entityType: string, id: number, companyId: number) => Promise<void>;
  purge: (entityType: string, id: number, companyId: number) => Promise<void>;
  reset: () => void;
}

export const useTrashStore = create<S>((set, get) => ({
  items: [],
  loadedForCompany: null,
  async loadFor(companyId) {
    const items = await call<TrashItem[]>("list_trash", { companyId });
    set({ items, loadedForCompany: companyId });
  },
  async restore(entityType, id, companyId) {
    await call<void>("restore_trash_item", { entityType, id });
    await get().loadFor(companyId);
  },
  async purge(entityType, id, companyId) {
    await call<void>("purge_trash_item", { entityType, id });
    await get().loadFor(companyId);
  },
  reset() {
    set({ items: [], loadedForCompany: null });
  },
}));
