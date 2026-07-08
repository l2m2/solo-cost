import { NavLink } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import { PAPER, INK, INK_SOFT, VERMILION, RULE, SERIF } from "@/lib/brand";
import { LayoutDashboard, FolderKanban, Users, Trash2, Settings, Contact } from "lucide-react";

const ITEMS = [
  { to: "/dashboard", icon: LayoutDashboard, key: "nav.dashboard" as const },
  { to: "/projects", icon: FolderKanban, key: "nav.projects" as const },
  { to: "/clients", icon: Contact, key: "nav.clients" as const },
  { to: "/members", icon: Users, key: "nav.members" as const },
  { to: "/trash", icon: Trash2, key: "nav.trash" as const },
  { to: "/settings", icon: Settings, key: "nav.settings" as const },
];

export function Sidebar() {
  const { t } = useTranslation();
  return (
    <aside
      className="flex w-56 flex-col"
      style={{ background: PAPER, color: INK, borderRight: `1px solid ${RULE}` }}
    >
      <div className="flex h-14 flex-col justify-center px-4" style={{ borderBottom: `1px solid ${RULE}` }}>
        <span className="text-lg font-semibold leading-none" style={SERIF}>{t("app.name")}</span>
        <span className="mt-1 text-[11px]" style={{ color: INK_SOFT }}>{t("app.tagline")}</span>
      </div>
      <nav className="flex-1 space-y-1 px-2 py-2">
        {ITEMS.map((it) => (
          <NavLink
            key={it.to}
            to={it.to}
            className={({ isActive }) =>
              cn(
                "relative flex items-center gap-2 rounded-md px-3 py-2 text-sm transition-colors hover:bg-black/[0.04]",
                isActive && "font-medium",
              )
            }
            style={({ isActive }) => ({ color: isActive ? VERMILION : INK_SOFT })}
          >
            {({ isActive }) => (
              <>
                {isActive && (
                  <span
                    className="absolute inset-y-1.5 left-0 w-0.5 rounded-full"
                    style={{ background: VERMILION }}
                  />
                )}
                <it.icon className="h-4 w-4" />
                <span>{t(it.key)}</span>
              </>
            )}
          </NavLink>
        ))}
      </nav>
    </aside>
  );
}
