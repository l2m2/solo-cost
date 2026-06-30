import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { Project, ProjectInput } from "@/types";

interface S {
  list: Project[];
  loadedForCompany: number | null;
  statusFilter: string | null;
  loadFor: (companyId: number, statusFilter?: string | null) => Promise<void>;
  create: (companyId: number, input: ProjectInput) => Promise<Project>;
  update: (id: number, input: ProjectInput) => Promise<Project>;
  setStatus: (id: number, status: string) => Promise<void>;
  softDelete: (id: number) => Promise<void>;
}

export const useProjectsStore = create<S>((set, get) => ({
  list: [],
  loadedForCompany: null,
  statusFilter: null,
  async loadFor(companyId, statusFilter = null) {
    const list = await call<Project[]>("list_projects", {
      companyId,
      status: statusFilter,
    });
    set({ list, loadedForCompany: companyId, statusFilter });
  },
  async create(companyId, input) {
    const p = await call<Project>("create_project", { companyId, input });
    set({ list: [p, ...get().list] });
    return p;
  },
  async update(id, input) {
    const p = await call<Project>("update_project", { id, input });
    set({ list: get().list.map((x) => (x.id === id ? p : x)) });
    return p;
  },
  async setStatus(id, status) {
    const p = await call<Project>("set_project_status", { id, status });
    set({ list: get().list.map((x) => (x.id === id ? p : x)) });
  },
  async softDelete(id) {
    await call<void>("delete_project", { id });
    set({ list: get().list.filter((x) => x.id !== id) });
  },
}));
