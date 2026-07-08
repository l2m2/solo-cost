import { Outlet } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { Header } from "./Header";

// A hair-warm workspace so the paper sidebar/header and the ledger dashboard
// panels sit on a tone that belongs to the same book; content cards stay light.
const WORKSPACE = "#FAF8F3";

export function AppLayout() {
  return (
    <div className="flex h-screen w-screen">
      <Sidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        <Header />
        <main className="flex-1 overflow-auto p-6" style={{ background: WORKSPACE }}>
          <Outlet />
        </main>
      </div>
    </div>
  );
}
