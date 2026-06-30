import { useEffect } from "react";
import { useCompanyStore } from "@/stores/company";

export default function DashboardPage() {
  const { list, currentId, loaded, loadAll } = useCompanyStore();
  useEffect(() => { if (!loaded) loadAll(); }, [loaded, loadAll]);

  const current = list.find((c) => c.id === currentId);
  return (
    <div className="space-y-4">
      <h1 className="text-xl font-semibold">仪表盘</h1>
      {current ? (
        <div className="text-sm text-muted-foreground">当前公司：{current.name}</div>
      ) : (
        <div className="text-sm text-muted-foreground">还没有公司，请先到「公司管理」创建。</div>
      )}
    </div>
  );
}
