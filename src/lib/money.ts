// All money values cross the IPC boundary as INTEGER cents.
// These helpers convert at the UI edge only.

export function toCents(yuan: string | number): number {
  if (yuan === "" || yuan === null || yuan === undefined) return 0;
  const n = typeof yuan === "number" ? yuan : Number(yuan);
  if (!Number.isFinite(n)) throw new RangeError(`invalid amount: ${yuan}`);
  if (n < 0) throw new RangeError("amount cannot be negative");
  return Math.round(n * 100);
}

export function fromCents(cents: number): string {
  return (cents / 100).toFixed(2);
}

const CNY = new Intl.NumberFormat("zh-CN", {
  style: "currency",
  currency: "CNY",
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

export function formatCNY(cents: number): string {
  return CNY.format(cents / 100);
}
