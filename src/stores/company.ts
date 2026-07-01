import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { Company, CompanyInput } from "@/types";

interface CompanyState {
  list: Company[];
  currentId: number | null;
  loaded: boolean;
  loadAll: () => Promise<void>;
  setCurrent: (id: number) => Promise<void>;
  create: (input: CompanyInput) => Promise<Company>;
  update: (id: number, input: CompanyInput) => Promise<Company>;
  reset: () => void;
}

export const useCompanyStore = create<CompanyState>((set, get) => ({
  list: [],
  currentId: null,
  loaded: false,
  async loadAll() {
    const [list, currentId] = await Promise.all([
      call<Company[]>("list_companies"),
      call<number | null>("get_current_company_id"),
    ]);
    let chosen = currentId;
    if (chosen === null && list.length > 0) {
      chosen = list[0].id;
      await call<void>("set_current_company", { id: chosen });
    }
    set({ list, currentId: chosen, loaded: true });
  },
  async setCurrent(id) {
    await call<void>("set_current_company", { id });
    set({ currentId: id });
  },
  async create(input) {
    const c = await call<Company>("create_company", { input });
    set({ list: [c, ...get().list] });
    if (get().currentId === null) await get().setCurrent(c.id);
    return c;
  },
  async update(id, input) {
    const c = await call<Company>("update_company", { id, input });
    set({ list: get().list.map((x) => (x.id === id ? c : x)) });
    return c;
  },
  reset() {
    set({ list: [], currentId: null, loaded: false });
  },
}));
