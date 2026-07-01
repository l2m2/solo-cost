import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { ContractPayment, PaymentInput } from "@/types";
import { useFinancialStore } from "./financial";

interface S {
  byProject: Record<number, ContractPayment[]>;
  loadFor: (projectId: number) => Promise<void>;
  create: (projectId: number, input: PaymentInput) => Promise<void>;
  update: (id: number, input: PaymentInput, projectId: number) => Promise<void>;
  markReceived: (id: number, actualAmountCents: number, actualReceivedAt: string, projectId: number) => Promise<void>;
  softDelete: (id: number, projectId: number) => Promise<void>;
  reset: () => void;
}

export const usePaymentsStore = create<S>((set, get) => ({
  byProject: {},
  async loadFor(projectId) {
    const list = await call<ContractPayment[]>("list_payments", { projectId });
    set({ byProject: { ...get().byProject, [projectId]: list } });
  },
  async create(projectId, input) {
    await call<ContractPayment>("create_payment", { projectId, input });
    await get().loadFor(projectId);
    await useFinancialStore.getState().refresh(projectId);
  },
  async update(id, input, projectId) {
    await call<ContractPayment>("update_payment", { id, input });
    await get().loadFor(projectId);
    await useFinancialStore.getState().refresh(projectId);
  },
  async markReceived(id, actualAmountCents, actualReceivedAt, projectId) {
    await call<ContractPayment>("mark_payment_received", {
      id,
      actualAmountCents,
      actualReceivedAt,
    });
    await get().loadFor(projectId);
    await useFinancialStore.getState().refresh(projectId);
  },
  async softDelete(id, projectId) {
    await call<void>("delete_payment", { id });
    await get().loadFor(projectId);
    await useFinancialStore.getState().refresh(projectId);
  },
  reset() {
    set({ byProject: {} });
  },
}));
