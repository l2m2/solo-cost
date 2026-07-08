import type { CSSProperties } from "react";

// Account-book brand palette for「沃工本」. Shared by the app shell (sidebar /
// header) and the ledger-styled dashboard. Kept as explicit values rather than
// global theme vars so content pages stay on the neutral shadcn theme.
export const PAPER = "#F7F4EC";
export const PAPER_BAR = "#E7DFCF"; // track behind ledger progress bars
export const INK = "#1C1A17";
export const INK_SOFT = "#6B6459";
export const VERMILION = "#B23A2E"; // outflow / overdue / active accent / the seal
export const INDIGO = "#3B4A6B";
export const RULE = "#D8CFBF"; // hairline rules and borders

// System Chinese serif stack — the brand's voice, no bundled web font.
export const SERIF: CSSProperties = {
  fontFamily: '"Songti SC","STSong","Source Han Serif SC","Noto Serif SC","SimSun",serif',
};
