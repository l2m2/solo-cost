import { NavLink } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import { LayoutDashboard, Building2, FolderKanban, Users, Tag, Trash2, Settings, Contact } from "lucide-react";

const ITEMS = [
  { to: "/dashboard", icon: LayoutDashboard, key: "nav.dashboard" as const },
  { to: "/projects", icon: FolderKanban, key: "nav.projects" as const },
  { to: "/clients", icon: Contact, key: "nav.clients" as const },
  { to: "/members", icon: Users, key: "nav.members" as const },
  { to: "/categories", icon: Tag, key: "nav.categories" as const },
  { to: "/companies", icon: Building2, key: "nav.companies" as const },
  { to: "/trash", icon: Trash2, key: "nav.trash" as const },
  { to: "/settings", icon: Settings, key: "nav.settings" as const },
];

export function Sidebar() {
  const { t } = useTranslation();
  return (
    <aside className="w-56 border-r bg-background flex flex-col">
      <div className="px-4 h-14 flex items-center font-semibold">{t("app.name")}</div>
      <nav className="flex-1 px-2 space-y-1">
        {ITEMS.map((it) => (
          <NavLink
            key={it.to}
            to={it.to}
            className={({ isActive }) =>
              cn(
                "flex items-center gap-2 px-3 py-2 rounded-md text-sm hover:bg-accent",
                isActive && "bg-accent",
              )
            }
          >
            <it.icon className="h-4 w-4" />
            <span>{t(it.key)}</span>
          </NavLink>
        ))}
      </nav>
    </aside>
  );
}
