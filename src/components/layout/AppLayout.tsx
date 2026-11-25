import { useEffect } from "react";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import { purgeExpiredTrash } from "@/lib/ipc";
import { useTrashStore } from "@/stores/trash";
import { Outlet } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { Header } from "./Header";
import { CommandPalette } from "@/components/search/CommandPalette";

// A hair-warm workspace so the paper sidebar/header and the ledger dashboard
// panels sit on a tone that belongs to the same book; content cards stay light.
const WORKSPACE = "#FAF8F3";

export function AppLayout() {
  const { t } = useTranslation();
  useEffect(() => {
    let active = true;
    const timer = window.setInterval(async () => {
      try {
        const removed = await purgeExpiredTrash();
        if (active && removed > 0) {
          const { loadedForCompany, loadFor } = useTrashStore.getState();
          if (loadedForCompany != null) await loadFor(loadedForCompany);
        }
      } catch (e) {
        if (active) toast.error(t("common.error", { msg: String(e) }));
      }
    }, 60 * 60 * 1000);
    return () => { active = false; window.clearInterval(timer); };
  }, [t]);

  return (
    <div className="flex h-screen w-screen">
      <Sidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        <Header />
        <main className="flex-1 overflow-auto p-6" style={{ background: WORKSPACE }}>
          <Outlet />
        </main>
      </div>
      <CommandPalette />
    </div>
  );
}
