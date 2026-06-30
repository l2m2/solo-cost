import { Button } from "@/components/ui/button";
import { CompanySwitcher } from "./CompanySwitcher";
import { useAuthStore } from "@/stores/auth";
import { LogOut } from "lucide-react";

export function Header() {
  const lock = useAuthStore((s) => s.lock);
  return (
    <header className="h-14 border-b px-6 flex items-center justify-between bg-background">
      <CompanySwitcher />
      <Button variant="ghost" size="sm" onClick={lock} className="gap-2">
        <LogOut className="h-4 w-4" />
        锁定
      </Button>
    </header>
  );
}
