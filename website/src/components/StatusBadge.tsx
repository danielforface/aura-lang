import type { ReactNode } from "react";

export type StatusTone = "proof" | "info" | "evolving" | "neutral" | "warning";
export function StatusBadge({ children, tone = "neutral" }: { children: ReactNode; tone?: StatusTone }) {
  return <span className={`status-badge status-${tone}`}>{children}</span>;
}
