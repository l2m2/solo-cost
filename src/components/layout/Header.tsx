import { Button } from "@/components/ui/button";
import { CompanySwitcher } from "./CompanySwitcher";
import { useAuthStore } from "@/stores/auth";
import { PAPER, RULE } from "@/lib/brand";
import { LogOut } from "lucide-react";

export function Header() {
  const lock = useAuthStore((s) => s.lock);
  return (
    <header
      className="flex h-14 items-center justify-between px-6"
      style={{ background: PAPER, borderBottom: `1px solid ${RULE}` }}
    >
      <CompanySwitcher />
      <Button variant="ghost" size="sm" onClick={lock} className="gap-2">
        <LogOut className="h-4 w-4" />
        锁定
      </Button>
    </header>
  );
}
