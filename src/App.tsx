import { useEffect } from "react";
import { BrowserRouter, Routes, Route, Navigate, useLocation } from "react-router-dom";
import { Toaster } from "@/components/ui/sonner";
import { useAuthStore } from "@/stores/auth";
import { AppLayout } from "@/components/layout/AppLayout";
import SetupPage from "@/routes/setup";
import LoginPage from "@/routes/login";
import DashboardPage from "@/routes/dashboard";
import CompaniesPage from "@/routes/companies";
import CategoriesPage from "@/routes/categories";
import ProjectsListPage from "@/routes/projects/list";
import ProjectDetailPage from "@/routes/projects/detail";
import TrashPage from "@/routes/trash";
import "@/i18n";

function AuthGate({ children }: { children: React.ReactNode }) {
  const status = useAuthStore((s) => s.status);
  const refresh = useAuthStore((s) => s.refresh);
  const location = useLocation();

  useEffect(() => {
    if (status === "unknown") refresh();
  }, [status, refresh]);

  if (status === "unknown") return null;
  if (status === "uninitialized" && location.pathname !== "/setup") return <Navigate to="/setup" replace />;
  if (status === "locked" && location.pathname !== "/login") return <Navigate to="/login" replace />;
  if (status === "unlocked" && (location.pathname === "/setup" || location.pathname === "/login")) {
    return <Navigate to="/dashboard" replace />;
  }
  return <>{children}</>;
}

export default function App() {
  return (
    <BrowserRouter>
      <AuthGate>
        <Routes>
          <Route path="/setup" element={<SetupPage />} />
          <Route path="/login" element={<LoginPage />} />
          <Route path="/" element={<AppLayout />}>
            <Route index element={<Navigate to="/dashboard" replace />} />
            <Route path="dashboard" element={<DashboardPage />} />
            <Route path="companies" element={<CompaniesPage />} />
            <Route path="projects" element={<ProjectsListPage />} />
            <Route path="projects/:id" element={<ProjectDetailPage />} />
            <Route path="categories" element={<CategoriesPage />} />
            <Route path="trash" element={<TrashPage />} />
            <Route path="settings" element={<div>设置（M4 实现）</div>} />
          </Route>
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </AuthGate>
      <Toaster richColors position="top-right" />
    </BrowserRouter>
  );
}
