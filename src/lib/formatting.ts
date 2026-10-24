/** Presentation-only formatting. Financial rules and totals are backend-owned. */

export function formatMoney(minor: number, currency = "USD"): string {
  return new Intl.NumberFormat(undefined, {
    style: "currency",
    currency,
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(minor / 100);
}

export function parseDateOnly(value: string): Date {
  const [year, month, day] = value.split("-").map(Number);
  return new Date(year, month - 1, day);
}

export function formatDateOnly(value: string): string {
  return parseDateOnly(value).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

export function formatDateShort(value: string): string {
  return parseDateOnly(value).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

export function formatMonthRange(start: string, endExclusive: string): string {
  const end = parseDateOnly(endExclusive);
  end.setDate(end.getDate() - 1);
  return `${formatDateShort(start)} – ${end.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  })}`;
}

export function monthLabel(month: string): string {
  const [year, value] = month.split("-").map(Number);
  return new Date(year, value - 1, 1).toLocaleDateString(undefined, {
    month: "long",
    year: "numeric",
  });
}

export function dueAmountLabel(
  kind: string,
  amountMinor: number | null,
  currency: string,
): string {
  switch (kind) {
    case "minimum_due":
      return `${formatMoney(amountMinor ?? 0, currency)} minimum`;
    case "reported_zero":
      return `${formatMoney(0, currency)} reported`;
    case "not_reported":
      return "amount not reported";
    default:
      return "no due date reported";
  }
}

export function connectionLabel(status: string | null, sourceKind: string): string {
  if (sourceKind === "manual") return "Manual";
  switch (status) {
    case "healthy":
      return "Connected";
    case "reconnect_required":
      return "Reconnect needed";
    case "error":
      return "Sync error";
    case "disconnected":
      return "Disconnected";
    default:
      return "Connection unknown";
  }
}
