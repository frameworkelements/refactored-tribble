import type { CertStatus } from "../types";

const LABELS: Record<CertStatus, string> = {
  valid: "Valid",
  expiring_soon: "Expiring soon",
  expired: "Expired",
};

export function StatusBadge({ status }: { status: CertStatus }) {
  return <span className={`badge ${status}`}>{LABELS[status]}</span>;
}
