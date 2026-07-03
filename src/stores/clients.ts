import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { Client, ClientInput } from "@/types";

interface S {
  list: Client[];
  loadedForCompany: number | null;
  loadFor: (companyId: number) => Promise<void>;
  create: (companyId: number, input: ClientInput) => Promise<Client>;
  update: (id: number, input: ClientInput) => Promise<Client>;
  softDelete: (id: number) => Promise<void>;
  reset: () => void;
}

const sortByName = (a: Client, b: Client) => a.name.localeCompare(b.name);

export const useClientsStore = create<S>((set, get) => ({
  list: [],
  loadedForCompany: null,
  async loadFor(companyId) {
    const list = await call<Client[]>("list_clients", { companyId });
    set({ list: [...list].sort(sortByName), loadedForCompany: companyId });
  },
  async create(companyId, input) {
    const c = await call<Client>("create_client", { companyId, input });
    set({ list: [...get().list, c].sort(sortByName) });
    return c;
  },
  async update(id, input) {
    const c = await call<Client>("update_client", { id, input });
    set({ list: get().list.map((x) => (x.id === id ? c : x)).sort(sortByName) });
    return c;
  },
  async softDelete(id) {
    await call<void>("delete_client", { id });
    set({ list: get().list.filter((x) => x.id !== id) });
  },
  reset() {
    set({ list: [], loadedForCompany: null });
  },
}));
