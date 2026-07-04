export function nowDatetimeLocal(): string {
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function todayIso(): string {
  const d = new Date();
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

export function formatHours(h: number): string {
  if (!Number.isFinite(h)) return "—";
  if (h === 0) return "0";
  if (h < 1) {
    const min = Math.round(h * 60);
    return `${min} 分钟`;
  }
  if (Number.isInteger(h)) return `${h} 小时`;
  return `${h.toFixed(2).replace(/\.?0+$/, "")} 小时`;
}
