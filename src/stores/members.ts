import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { Member, MemberInput } from "@/types";

interface S {
  list: Member[];
  loadedForCompany: number | null;
  loadFor: (companyId: number) => Promise<void>;
  create: (companyId: number, input: MemberInput) => Promise<Member>;
  update: (id: number, input: MemberInput) => Promise<Member>;
  setActive: (id: number, isActive: boolean) => Promise<void>;
  softDelete: (id: number) => Promise<void>;
  reset: () => void;
}

export const useMembersStore = create<S>((set, get) => ({
  list: [],
  loadedForCompany: null,
  async loadFor(companyId) {
    const list = await call<Member[]>("list_members", { companyId });
    // Sort: active first (is_active DESC), then by name
    const sorted = [...list].sort((a, b) => {
      if (a.is_active !== b.is_active) return a.is_active ? -1 : 1;
      return a.name.localeCompare(b.name);
    });
    set({ list: sorted, loadedForCompany: companyId });
  },
  async create(companyId, input) {
    const m = await call<Member>("create_member", { companyId, input });
    // Insert active members at top, inactive at bottom
    const list = get().list;
    const newList = m.is_active ? [m, ...list] : [...list, m];
    set({ list: newList });
    return m;
  },
  async update(id, input) {
    const m = await call<Member>("update_member", { id, input });
    set({ list: get().list.map((x) => (x.id === id ? m : x)) });
    return m;
  },
  async setActive(id, isActive) {
    const m = await call<Member>("set_member_active", { id, isActive });
    // Re-sort after active status changes
    const updated = get().list.map((x) => (x.id === id ? m : x));
    const sorted = [...updated].sort((a, b) => {
      if (a.is_active !== b.is_active) return a.is_active ? -1 : 1;
      return a.name.localeCompare(b.name);
    });
    set({ list: sorted });
  },
  async softDelete(id) {
    await call<void>("delete_member", { id });
    set({ list: get().list.filter((x) => x.id !== id) });
  },
  reset() {
    set({ list: [], loadedForCompany: null });
  },
}));
