export const STATUS_VALUES = [
  "negotiating",
  "pending",
  "in_progress",
  "delivered",
  "settled",
  "archived",
] as const;

export type ProjectStatus = typeof STATUS_VALUES[number];

const LABELS: Record<ProjectStatus, string> = {
  negotiating: "商务洽谈",
  pending: "待启动",
  in_progress: "进行中",
  delivered: "已交付待结款",
  settled: "已结款",
  archived: "已归档",
};

const BADGE_CLASSES: Record<ProjectStatus, string> = {
  negotiating: "bg-slate-100 text-slate-700",
  pending: "bg-blue-100 text-blue-700",
  in_progress: "bg-amber-100 text-amber-700",
  delivered: "bg-purple-100 text-purple-700",
  settled: "bg-emerald-100 text-emerald-700",
  archived: "bg-zinc-100 text-zinc-500",
};

export function statusLabel(s: string): string {
  return LABELS[s as ProjectStatus] ?? s;
}

export function statusBadgeClass(s: string): string {
  return BADGE_CLASSES[s as ProjectStatus] ?? "bg-zinc-100 text-zinc-700";
}

export const STATUS_OPTIONS = STATUS_VALUES.map((v) => ({
  value: v,
  label: LABELS[v],
}));
