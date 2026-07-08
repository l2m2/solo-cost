import type { TFunction } from "i18next";
import { formatCNY } from "@/lib/money";
import type { DashboardSummary, DashYearRow } from "@/types";
import { PAPER, PAPER_BAR, INK, INK_SOFT, VERMILION, INDIGO, RULE, SERIF } from "./ledgerTokens";

function clampPct(value: number, max: number): string {
  return `${Math.max(0, Math.min(100, (value / max) * 100))}%`;
}

// The signature: a vermilion seal stamped beside the hero figure.
function Seal({ label }: { label: string }) {
  return (
    <div className="-rotate-6 select-none" aria-hidden>
      <div
        className="relative flex h-16 w-16 items-center justify-center rounded-full"
        style={{ border: `2px solid ${VERMILION}`, color: VERMILION }}
      >
        <span
          className="absolute inset-1 rounded-full"
          style={{ border: `1px solid ${VERMILION}`, opacity: 0.7 }}
        />
        <span className="text-3xl leading-none" style={SERIF}>{label}</span>
      </div>
    </div>
  );
}

type LedgerRow = { label: string; value: number; strong?: boolean; negative?: boolean };

function LedgerBlock({ title, rows }: { title: string; rows: LedgerRow[] }) {
  return (
    <div>
      <div className="mb-1 text-xs font-medium" style={{ color: INK_SOFT }}>{title}</div>
      <dl>
        {rows.map((r) => (
          <div
            key={r.label}
            className={`flex items-baseline justify-between py-1.5 ${r.strong ? "border-t" : ""}`}
            style={r.strong ? { borderColor: INK, borderTopWidth: 2 } : undefined}
          >
            <dt className="text-sm" style={{ color: r.strong ? INK : INK_SOFT }}>{r.label}</dt>
            <dd
              className={`tabular-nums ${r.strong ? "text-lg font-semibold" : "text-sm"}`}
              style={{ ...SERIF, color: r.negative ? VERMILION : INK }}
            >
              {r.negative ? "−" : ""}{formatCNY(r.value)}
            </dd>
          </div>
        ))}
      </dl>
    </div>
  );
}

export function LedgerOverview({ data, t, onOpenYear }: {
  data: DashboardSummary;
  t: TFunction;
  onOpenYear: (y: DashYearRow) => void;
}) {
  const maxYear = Math.max(
    1,
    ...data.by_year.map((y) => Math.max(y.net_cents, y.received_exclusive_cents)),
  );

  return (
    <div
      className="space-y-6 rounded-lg p-6"
      style={{ background: PAPER, color: INK, border: `1px solid ${RULE}` }}
    >
      {/* Ledger masthead */}
      <div
        className="flex items-baseline justify-between pb-2"
        style={{ borderBottom: `3px double ${INK}` }}
      >
        <h2 className="text-base font-semibold tracking-wide" style={SERIF}>
          {t("dashboard.ledgerTitle")}
        </h2>
        <span className="text-xs" style={{ color: INK_SOFT }}>{t("dashboard.subtitle")}</span>
      </div>

      {/* Hero: net realized + seal */}
      <div className="flex items-start justify-between">
        <div>
          <div className="text-xs" style={{ color: INK_SOFT }}>{t("dashboard.netRealized")}</div>
          <div className="mt-1 text-5xl font-semibold tabular-nums" style={SERIF}>
            {formatCNY(data.net_realized_cents)}
          </div>
          <div className="mt-2 text-xs" style={{ color: INK_SOFT }}>
            {t("dashboard.received")} {formatCNY(data.received_inclusive_cents)}
            <span style={{ color: VERMILION }}> − {t("financial.commission")} {formatCNY(data.commission_realized_cents)}</span>
            <span style={{ color: VERMILION }}> − {t("financial.generalCost")} {formatCNY(data.general_cost_cents)}</span>
          </div>
        </div>
        <Seal label="沃" />
      </div>

      {/* Two ledger scopes */}
      <div className="grid grid-cols-2 gap-10">
        <LedgerBlock
          title={t("dashboard.contractScope")}
          rows={[
            { label: t("dashboard.contractTotal"), value: data.contract_total_inclusive_cents },
            { label: t("dashboard.revenueExclusive"), value: data.revenue_exclusive_cents },
            { label: t("dashboard.netPotential"), value: data.net_potential_cents, strong: true },
          ]}
        />
        <LedgerBlock
          title={t("dashboard.receivedScope")}
          rows={[
            { label: t("dashboard.received"), value: data.received_inclusive_cents },
            { label: t("dashboard.outstanding"), value: data.outstanding_cents, negative: true },
            { label: t("dashboard.netRealized"), value: data.net_realized_cents, strong: true },
          ]}
        />
      </div>

      {/* Net by year */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-medium" style={SERIF}>{t("dashboard.netByYear")}</h3>
          <div className="flex items-center gap-4 text-xs" style={{ color: INK_SOFT }}>
            <span className="flex items-center gap-1.5">
              <span className="h-2 w-2 rounded-full" style={{ background: INK_SOFT }} />
              {t("dashboard.receivedLabel")}
            </span>
            <span className="flex items-center gap-1.5">
              <span className="h-2 w-2 rounded-full" style={{ background: INDIGO }} />
              {t("dashboard.netLabel")}
            </span>
          </div>
        </div>
        {data.by_year.length === 0 ? (
          <div className="text-sm" style={{ color: INK_SOFT }}>{t("dashboard.empty")}</div>
        ) : data.by_year.map((y) => (
          <button
            key={y.year}
            type="button"
            className="block w-full space-y-1.5 rounded p-1.5 text-left transition-colors hover:bg-black/[0.04]"
            title={t("dashboard.viewYearDetail")}
            onClick={() => onOpenYear(y)}
          >
            <div className="flex justify-between text-xs">
              <span className="font-medium tabular-nums">{y.year}</span>
              <span className="tabular-nums" style={{ color: INK_SOFT }}>
                {t("dashboard.netLabel")} {formatCNY(y.net_cents)} · {t("dashboard.receivedLabel")} {formatCNY(y.received_exclusive_cents)}
              </span>
            </div>
            <LedgerBar value={y.received_exclusive_cents} max={maxYear} color={INK_SOFT} />
            <LedgerBar value={y.net_cents} max={maxYear} color={INDIGO} />
          </button>
        ))}
      </div>
    </div>
  );
}

function LedgerBar({ value, max, color }: { value: number; max: number; color: string }) {
  return (
    <div className="h-2 overflow-hidden rounded-full" style={{ background: PAPER_BAR }}>
      <div className="h-full rounded-full" style={{ width: clampPct(value, max), background: color }} />
    </div>
  );
}
