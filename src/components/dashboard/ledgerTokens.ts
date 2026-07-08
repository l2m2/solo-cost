import type { CSSProperties } from "react";

// Shared "account book" design tokens for the ledger-styled dashboard sections.
// Kept as explicit values (not global theme vars) so the ledger look stays
// scoped to the dashboard and never leaks into the rest of the app.
export const PAPER = "#F7F4EC";
export const PAPER_BAR = "#E7DFCF"; // track behind ledger progress bars
export const INK = "#1C1A17";
export const INK_SOFT = "#6B6459";
export const VERMILION = "#B23A2E"; // outflow / overdue / the seal
export const INDIGO = "#3B4A6B";
export const RULE = "#D8CFBF"; // hairline rules and panel borders

// System Chinese serif stack — the account book's voice, no bundled web font.
export const SERIF: CSSProperties = {
  fontFamily: '"Songti SC","STSong","Source Han Serif SC","Noto Serif SC","SimSun",serif',
};
