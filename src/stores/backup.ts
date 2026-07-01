import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { BackupInfo, BackupStatus } from "@/types";

interface S {
  status: BackupStatus | null;
  list: BackupInfo[];
  loadStatus: () => Promise<void>;
  loadList: () => Promise<void>;
  createNow: () => Promise<BackupInfo>;
  maybeAutoBackup: () => Promise<BackupInfo | null>;
  exportPlaintext: (dstPath: string) => Promise<void>;
  restoreFromBackup: (backupPath: string, password: string) => Promise<void>;
  reset: () => void;
}

export const useBackupStore = create<S>((set, get) => ({
  status: null,
  list: [],
  async loadStatus() {
    const status = await call<BackupStatus>("get_backup_status");
    set({ status });
  },
  async loadList() {
    const list = await call<BackupInfo[]>("list_backups");
    set({ list });
  },
  async createNow() {
    const info = await call<BackupInfo>("create_backup_now");
    await get().loadStatus();
    await get().loadList();
    return info;
  },
  async maybeAutoBackup() {
    const info = await call<BackupInfo | null>("maybe_run_auto_backup");
    if (info) {
      await get().loadStatus();
      await get().loadList();
    }
    return info;
  },
  async exportPlaintext(dstPath) {
    await call<void>("export_plaintext_backup", { dstPath });
  },
  async restoreFromBackup(backupPath, password) {
    await call<void>("restore_from_backup", { backupPath, password });
    // do NOT reload here; caller must trigger lock + unlock to reset all stores
  },
  reset() {
    set({ status: null, list: [] });
  },
}));
