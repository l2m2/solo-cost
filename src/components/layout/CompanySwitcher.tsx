import { useEffect } from "react";
import { Check, ChevronDown, Building2 } from "lucide-react";
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Button } from "@/components/ui/button";
import { useCompanyStore } from "@/stores/company";

export function CompanySwitcher() {
  const { list, currentId, loaded, loadAll, setCurrent } = useCompanyStore();
  useEffect(() => { if (!loaded) loadAll(); }, [loaded, loadAll]);

  const current = list.find((c) => c.id === currentId);

  if (!loaded) return null;
  if (list.length === 0) return <span className="text-sm text-muted-foreground">尚未创建公司</span>;

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="outline" size="sm" className="gap-2">
          <Building2 className="h-4 w-4" />
          {current?.name ?? "选择公司"}
          <ChevronDown className="h-4 w-4 opacity-60" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="min-w-48">
        {list.map((c) => (
          <DropdownMenuItem key={c.id} onClick={() => setCurrent(c.id)} className="gap-2">
            {c.id === currentId && <Check className="h-4 w-4" />}
            <span className={c.id === currentId ? "font-medium" : undefined}>{c.name}</span>
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
