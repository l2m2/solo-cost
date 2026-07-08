import type { ReactNode } from "react";
import { PAPER, PAPER_BAR, INK, RULE, SERIF } from "./ledgerTokens";

// A paper panel with an optional ruled header, shared by the ledger-styled
// dashboard sections so every "page" of the account book matches.
export function LedgerPanel({ title, right, bodyClassName, children }: {
  title?: ReactNode;
  right?: ReactNode;
  bodyClassName?: string;
  children: ReactNode;
}) {
  return (
    <div className="overflow-hidden rounded-lg" style={{ background: PAPER, color: INK, border: `1px solid ${RULE}` }}>
      {(title || right) && (
        <div className="flex items-center justify-between px-5 py-3" style={{ borderBottom: `1px solid ${RULE}` }}>
          {title ? <h3 className="text-sm font-medium" style={SERIF}>{title}</h3> : <span />}
          {right}
        </div>
      )}
      <div className={bodyClassName}>{children}</div>
    </div>
  );
}

export function LedgerBar({ value, max, color }: { value: number; max: number; color: string }) {
  const width = `${Math.max(0, Math.min(100, (value / max) * 100))}%`;
  return (
    <div className="h-2 overflow-hidden rounded-full" style={{ background: PAPER_BAR }}>
      <div className="h-full rounded-full" style={{ width, background: color }} />
    </div>
  );
}
