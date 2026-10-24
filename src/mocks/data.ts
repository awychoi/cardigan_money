/**
 * Browser-mock data and contract-shaped responses. This module stands in for
 * the Rust backend in development and Playwright only; it is aliased out of
 * production builds. Financial rules here are test-fixture logic, not the
 * renderer's source of truth.
 */
import type {
  Account,
  AccountCard,
  AccountSpend,
  Breakdown,
  CurrencySubtotal,
  Dashboard,
  LedgerPage,
  LedgerQuery,
  LedgerRow,
  MonthSummary,
  Overview,
  PaymentInfo,
  UpcomingPayment,
} from "../lib/bridge/contract";
import { BridgeError } from "../lib/bridge/errors";

const TODAY = "2026-10-23";
const CURRENT_MONTH = "2026-10";

interface MockTx {
  id: string;
  accountId: string;
  date: string;
  amountMinor: number;
  txType: string;
  merchant: string;
  category: string | null;
  pending: boolean;
  excluded: boolean;
  currency: string;
  authorizedDate: string | null;
  postedDate: string | null;
}

const ACCOUNT_SEED: Account[] = [
  account("aaaaaaaa-0000-4000-8000-000000000001", "Chase", "Sapphire Reserve (demo)", "4101"),
  account("aaaaaaaa-0000-4000-8000-000000000002", "Chase", "Freedom Unlimited (demo)", "4102"),
  account("aaaaaaaa-0000-4000-8000-000000000003", "Synchrony", "Amazon Store Card (demo)", "5501"),
  account("aaaaaaaa-0000-4000-8000-000000000004", "Bilt", "Bilt Mastercard (demo)", "6601"),
  account("aaaaaaaa-0000-4000-8000-000000000005", "American Express", "Platinum (demo)", "7701"),
  account("aaaaaaaa-0000-4000-8000-000000000006", "Citi", "Double Cash (demo)", "8801"),
  account("aaaaaaaa-0000-4000-8000-000000000007", "Demo Bank", "Euro Travel (demo)", "9901", "EUR"),
  account("aaaaaaaa-0000-4000-8000-000000000008", "Synchrony", "PayPal Credit (demo)", "5502"),
];

function account(
  id: string,
  issuer: string,
  displayName: string,
  lastFour: string,
  currency = "USD",
): Account {
  return {
    id,
    issuer,
    displayName,
    lastFour,
    accountType: "credit_card",
    currency,
    creditLimitMinor: 1_000_000,
    includeInOverview: true,
    archived: false,
    sourceKind: "demo",
    createdAt: "2026-10-01T00:00:00Z",
    updatedAt: "2026-10-01T00:00:00Z",
  };
}

let accounts: Account[] = structuredClone(ACCOUNT_SEED);
let transactions: MockTx[] = [];

export function resetMockData(): void {
  accounts = structuredClone(ACCOUNT_SEED);
  transactions = seedTransactions();
  overrides.clear();
  merchantRules.length = 0;
  manualPayments.clear();
  importBatches.length = 0;
  importJob = null;
}

export function mockAccounts(): Account[] {
  return structuredClone(accounts);
}

export function createMockAccount(input: {
  issuer: string;
  displayName: string;
  lastFour: string | null;
  accountType: Account["accountType"];
  currency: string;
  creditLimit: string | null;
  includeInOverview: boolean;
}): Account {
  const now = "2026-10-23T12:00:00Z";
  const created: Account = {
    id: crypto.randomUUID(),
    issuer: input.issuer,
    displayName: input.displayName,
    lastFour: input.lastFour,
    accountType: input.accountType,
    currency: input.currency,
    creditLimitMinor: input.creditLimit
      ? Math.round(Number(input.creditLimit.replace(/[$,]/g, "")) * 100)
      : null,
    includeInOverview: input.includeInOverview,
    archived: false,
    sourceKind: "manual",
    createdAt: now,
    updatedAt: now,
  };
  accounts = [...accounts, created];
  return structuredClone(created);
}

export function updateMockAccount(
  id: string,
  input: Parameters<typeof createMockAccount>[0],
): Account {
  const existing = accounts.find((item) => item.id === id);
  if (!existing) throw new BridgeError("not_found", "Account not found.");
  const updated: Account = {
    ...existing,
    issuer: input.issuer,
    displayName: input.displayName,
    lastFour: input.lastFour,
    accountType: input.accountType,
    currency: input.currency,
    creditLimitMinor: input.creditLimit
      ? Math.round(Number(input.creditLimit.replace(/[$,]/g, "")) * 100)
      : null,
    includeInOverview: input.includeInOverview,
    updatedAt: "2026-10-23T12:00:00Z",
  };
  accounts = accounts.map((item) => (item.id === id ? updated : item));
  return structuredClone(updated);
}

export function archiveMockAccount(id: string, archived: boolean): Account {
  const existing = accounts.find((item) => item.id === id);
  if (!existing) throw new BridgeError("not_found", "Account not found.");
  const updated: Account = { ...existing, archived, updatedAt: "2026-10-23T12:00:00Z" };
  accounts = accounts.map((item) => (item.id === id ? updated : item));
  return structuredClone(updated);
}

function makeTx(
  id: string,
  accountId: string,
  date: string,
  amountMinor: number,
  txType: string,
  merchant: string,
  category: string | null,
  options: {
    pending?: boolean;
    excluded?: boolean;
    currency?: string;
    authorizedDate?: string | null;
    postedDate?: string | null;
  } = {},
): MockTx {
  return {
    id,
    accountId,
    date,
    amountMinor,
    txType,
    merchant,
    category,
    pending: options.pending ?? false,
    excluded: options.excluded ?? false,
    currency: options.currency ?? "USD",
    authorizedDate: options.authorizedDate ?? null,
    postedDate: options.postedDate ?? date,
  };
}

function seedTransactions(): MockTx[] {
  const rows: MockTx[] = [];
  const merchants: [string, string][] = [
    ["Whole Foods Market", "Groceries"],
    ["Blue Bottle Coffee", "Dining"],
    ["Uber", "Transport"],
    ["Amazon", "Shopping"],
    ["Netflix", "Subscriptions"],
    ["Shell", "Gas"],
    ["CVS Pharmacy", "Health"],
    ["Delta Air Lines", "Travel"],
  ];
  let index = 1;
  for (const card of ["1", "2", "3", "4", "5", "6"]) {
    for (const month of ["08", "09", "10"]) {
      for (const offset of [0, 1]) {
        const [merchant, category] =
          merchants[(Number(card) + Number(month) + offset) % merchants.length];
        const day = String(4 + ((Number(card) * 3 + Number(month) + offset * 7) % 19)).padStart(
          2,
          "0",
        );
        const amount = 500 + ((Number(card) * 397 + Number(month) * 211 + offset * 997) % 18_000);
        rows.push(
          makeTx(
            `mock-tx-${index++}`,
            `aaaaaaaa-0000-4000-8000-00000000000${card}`,
            `2026-${month}-${day}`,
            amount,
            "purchase",
            merchant,
            category,
          ),
        );
      }
    }
  }
  // Month boundary: authorized Sep 30, posted Oct 2.
  rows.push(
    makeTx(
      "mock-boundary",
      accountId(2),
      "2026-09-30",
      21_540,
      "purchase",
      "Delta Air Lines",
      "Travel",
      {
        authorizedDate: "2026-09-30",
        postedDate: "2026-10-02",
      },
    ),
  );
  // Later-month refund.
  rows.push(
    makeTx(
      "mock-refund-buy",
      accountId(2),
      "2026-08-12",
      30_000,
      "purchase",
      "Apple Store",
      "Electronics",
    ),
  );
  rows.push(
    makeTx(
      "mock-refund",
      accountId(2),
      "2026-10-03",
      -30_000,
      "refund",
      "Apple Store",
      "Electronics",
    ),
  );
  // Pending + posted pair.
  rows.push(
    makeTx(
      "mock-pending",
      accountId(1),
      "2026-10-22",
      4_250,
      "purchase",
      "Blue Bottle Coffee",
      "Dining",
      {
        pending: true,
        postedDate: null,
        authorizedDate: "2026-10-22",
      },
    ),
  );
  rows.push(
    makeTx(
      "mock-posted",
      accountId(1),
      "2026-10-23",
      4_250,
      "purchase",
      "Blue Bottle Coffee",
      "Dining",
    ),
  );
  // Identical purchases.
  rows.push(
    makeTx(
      "mock-dup-1",
      accountId(2),
      "2026-10-12",
      1_234,
      "purchase",
      "Blue Bottle Coffee",
      "Dining",
    ),
  );
  rows.push(
    makeTx(
      "mock-dup-2",
      accountId(1),
      "2026-10-12",
      1_234,
      "purchase",
      "Blue Bottle Coffee",
      "Dining",
    ),
  );
  // Payments/transfers/exclusion/fees/interest.
  rows.push(
    makeTx(
      "mock-payment",
      accountId(1),
      "2026-10-15",
      -100_000,
      "payment",
      "Payment Thank You",
      null,
    ),
  );
  rows.push(
    makeTx(
      "mock-transfer",
      accountId(2),
      "2026-10-16",
      50_000,
      "transfer",
      "Balance Transfer",
      null,
    ),
  );
  rows.push(makeTx("mock-fee", accountId(1), "2026-10-01", 9_500, "fee", "Annual Fee", "Fees"));
  rows.push(
    makeTx(
      "mock-interest",
      accountId(6),
      "2026-10-05",
      1_822,
      "interest",
      "Interest Charge",
      "Interest",
    ),
  );
  rows.push(
    makeTx("mock-excluded", accountId(3), "2026-10-09", 7_999, "purchase", "Amazon", "Shopping", {
      excluded: true,
    }),
  );
  // Non-USD rows.
  for (let i = 0; i < 4; i += 1) {
    rows.push(
      makeTx(
        `mock-eur-${i}`,
        accountId(7),
        `2026-10-${String(4 + i * 4).padStart(2, "0")}`,
        2_100 + i * 900,
        "purchase",
        "Café de Flore",
        "Dining",
        {
          currency: "EUR",
        },
      ),
    );
  }
  return rows;
}

function accountId(card: number): string {
  return `aaaaaaaa-0000-4000-8000-00000000000${card}`;
}

function shiftMonth(month: string, delta: number): string {
  const [year, mon] = month.split("-").map(Number);
  const index = year * 12 + (mon - 1) + delta;
  const nextYear = Math.floor(index / 12);
  const nextMonth = (index % 12) + 1;
  return `${String(nextYear).padStart(4, "0")}-${String(nextMonth).padStart(2, "0")}`;
}

function monthBounds(month: string): { start: string; end: string } {
  const next = shiftMonth(month, 1);
  return { start: `${month}-01`, end: `${next}-01` };
}

function inMonth(date: string, month: string): boolean {
  return date.startsWith(month);
}

function isSpend(type: string): boolean {
  return type === "purchase" || type === "refund" || type === "fee" || type === "interest";
}

function eligible(tx: MockTx): boolean {
  const eff = effective(tx);
  return !tx.pending && !eff.excluded && isSpend(eff.type);
}

function breakdown(rows: MockTx[], keyOf: (tx: MockTx) => string): Breakdown[] {
  const map = new Map<string, Breakdown>();
  for (const tx of rows) {
    const key = keyOf(tx);
    const item = map.get(key) ?? { key, grossMinor: 0, refundsMinor: 0, netMinor: 0, count: 0 };
    if (tx.amountMinor > 0) item.grossMinor += tx.amountMinor;
    if (tx.amountMinor < 0) item.refundsMinor += tx.amountMinor;
    item.netMinor += tx.amountMinor;
    item.count += 1;
    map.set(key, item);
  }
  return [...map.values()].sort((a, b) => b.netMinor - a.netMinor || a.key.localeCompare(b.key));
}

function summaryFor(month: string): MonthSummary {
  const bounds = monthBounds(month);
  const usd = transactions.filter((tx) => tx.currency === "USD" && inMonth(tx.date, month));
  const spend = usd.filter(eligible);
  const grossPurchasesMinor = spend
    .filter((tx) => effective(tx).type === "purchase")
    .reduce((sum, tx) => sum + tx.amountMinor, 0);
  const refundsMinor = spend
    .filter((tx) => effective(tx).type === "refund")
    .reduce((sum, tx) => sum + tx.amountMinor, 0);
  const feesInterestMinor = spend
    .filter((tx) => ["fee", "interest"].includes(effective(tx).type))
    .reduce((sum, tx) => sum + tx.amountMinor, 0);
  const netSpendingMinor = spend.reduce((sum, tx) => sum + tx.amountMinor, 0);
  const pendingSubtotalMinor = usd
    .filter((tx) => tx.pending && !effective(tx).excluded && isSpend(effective(tx).type))
    .reduce((sum, tx) => sum + tx.amountMinor, 0);
  const suppliers = breakdown(spend, (tx) => effective(tx).merchant);
  const categories = breakdown(spend, (tx) => effective(tx).category ?? "Uncategorized");
  const supplierNet = suppliers.reduce((sum, item) => sum + item.netMinor, 0);
  const categoryNet = categories.reduce((sum, item) => sum + item.netMinor, 0);
  return {
    month,
    startDate: bounds.start,
    endDateExclusive: bounds.end,
    currency: "USD",
    grossPurchasesMinor,
    refundsMinor,
    feesInterestMinor,
    netSpendingMinor,
    pendingSubtotalMinor,
    eligibleCount: spend.length,
    pendingCount: usd.filter((tx) => tx.pending).length,
    rowCount: usd.length,
    otherCurrencyRowCount: transactions.filter(
      (tx) => tx.currency !== "USD" && inMonth(tx.date, month),
    ).length,
    suppliers,
    categories,
    reconciliation: {
      eligibleNetMinor: netSpendingMinor,
      supplierNetMinor: supplierNet,
      categoryNetMinor: categoryNet,
      exact: supplierNet === netSpendingMinor && categoryNet === netSpendingMinor,
    },
  };
}

const BALANCES: Record<string, { minor: number; retrieved: string; stale?: boolean }> = {
  [accountId(1)]: { minor: 432_100, retrieved: "2026-10-23T08:00:00Z" },
  [accountId(2)]: { minor: 128_450, retrieved: "2026-10-23T08:00:00Z" },
  [accountId(3)]: { minor: 54_300, retrieved: "2026-10-23T08:00:00Z" },
  [accountId(4)]: { minor: 91_750, retrieved: "2026-10-23T08:00:00Z" },
  [accountId(5)]: { minor: 210_000, retrieved: "2026-10-13T08:00:00Z", stale: true },
  [accountId(6)]: { minor: 33_200, retrieved: "2026-10-23T08:00:00Z" },
  [accountId(7)]: { minor: 4_500, retrieved: "2026-10-23T08:00:00Z" },
  // PayPal has no balance snapshot (unknown, not zero).
};

const PAYMENTS: Record<
  string,
  {
    due: string | null;
    minimum: number | null;
    statement: number | null;
    source: string;
    manual?: boolean;
  }
> = {
  [accountId(1)]: { due: "2026-10-28", minimum: 35_000, statement: 432_100, source: "provider" },
  [accountId(2)]: { due: "2026-11-03", minimum: 25_000, statement: 128_450, source: "provider" },
  [accountId(3)]: { due: "2026-10-26", minimum: null, statement: 54_300, source: "provider" },
  [accountId(4)]: { due: "2026-10-30", minimum: 9_000, statement: 91_750, source: "provider" },
  [accountId(5)]: { due: "2026-11-05", minimum: 21_000, statement: 210_000, source: "provider" },
  [accountId(6)]: {
    due: "2026-10-29",
    minimum: 3_300,
    statement: 33_200,
    source: "manual",
    manual: true,
  },
  [accountId(8)]: { due: "2026-10-29", minimum: 0, statement: 0, source: "provider" },
};

const CONNECTION: Record<string, { status: string; issuer: string }> = {
  [accountId(1)]: { status: "healthy", issuer: "Chase (demo)" },
  [accountId(2)]: { status: "healthy", issuer: "Chase (demo)" },
  [accountId(3)]: { status: "reconnect_required", issuer: "Synchrony (demo)" },
  [accountId(4)]: { status: "healthy", issuer: "Bilt (demo)" },
  [accountId(5)]: { status: "error", issuer: "American Express (demo)" },
  [accountId(6)]: { status: "healthy", issuer: "Citi (demo)" },
  [accountId(8)]: { status: "healthy", issuer: "Synchrony (demo)" },
};

function paymentInfo(accountIdValue: string): PaymentInfo {
  const payment = PAYMENTS[accountIdValue];
  if (!payment) {
    return {
      nextDueDate: null,
      dueAmountMinor: null,
      dueAmountKind: "no_due_date",
      dueSource: null,
      statementBalanceMinor: null,
      statementBalanceSource: null,
      statementIssueDate: null,
      nextClosingDate: null,
      validThroughDate: null,
      manualExpired: false,
      retrievedAt: "2026-10-23T08:00:00Z",
    };
  }
  const kind =
    payment.minimum === null
      ? "not_reported"
      : payment.minimum === 0
        ? "reported_zero"
        : "minimum_due";
  return {
    nextDueDate: payment.due,
    dueAmountMinor: payment.minimum,
    dueAmountKind: kind,
    dueSource: payment.source,
    statementBalanceMinor: payment.statement,
    statementBalanceSource: payment.source,
    statementIssueDate: "2026-10-05",
    nextClosingDate: null,
    validThroughDate: payment.manual ? "2026-11-15" : null,
    manualExpired: false,
    retrievedAt: "2026-10-23T08:00:00Z",
  };
}

function accountCards(): AccountCard[] {
  return accounts
    .filter((item) => !item.archived)
    .map((item) => {
      const balance = BALANCES[item.id];
      const connection = CONNECTION[item.id];
      return {
        accountId: item.id,
        issuer: item.issuer,
        displayName: item.displayName,
        lastFour: item.lastFour,
        currency: item.currency,
        included: item.includeInOverview,
        sourceKind: item.sourceKind,
        connectionStatus: connection?.status ?? null,
        currentBalanceMinor: balance?.minor ?? null,
        availableCreditMinor: balance ? 500_000 : null,
        creditLimitMinor: balance ? 1_000_000 : null,
        balanceRetrievedAt: balance?.retrieved ?? null,
        balanceProvenance: balance ? "provider" : null,
        balanceAgeDays: balance?.stale ? 10 : balance ? 0 : null,
        balanceStale: balance?.stale ?? false,
        payment: paymentInfo(item.id),
      };
    });
}

function overviewFor(): Overview {
  const cards = accountCards();
  const includedUsd = cards.filter((card) => card.included && card.currency === "USD");
  const withBalance = includedUsd.filter((card) => card.currentBalanceMinor !== null);
  const knownTotalMinor = withBalance.reduce(
    (sum, card) => sum + (card.currentBalanceMinor ?? 0),
    0,
  );
  const entries: UpcomingPayment[] = [];
  for (const card of cards) {
    if (!card.included || !card.payment.nextDueDate) continue;
    if (card.payment.nextDueDate >= TODAY && card.payment.nextDueDate < "2026-11-06") {
      entries.push({
        accountId: card.accountId,
        displayName: card.displayName,
        lastFour: card.lastFour,
        currency: card.currency,
        dueDate: card.payment.nextDueDate,
        amountMinor: card.payment.dueAmountMinor,
        amountKind: card.payment.dueAmountKind,
        source: card.payment.dueSource ?? "",
      });
    }
  }
  return {
    accounts: cards,
    balances: {
      knownTotalMinor,
      accountsWithBalance: withBalance.length,
      accountsMissingBalance: includedUsd.length - withBalance.length,
      complete: includedUsd.length > 0 && includedUsd.length === withBalance.length,
      otherCurrency: cards
        .filter(
          (card) => card.included && card.currency !== "USD" && card.currentBalanceMinor !== null,
        )
        .map((card) => [card.currency, card.currentBalanceMinor ?? 0] as [string, number]),
    },
    upcoming: {
      asOfDate: TODAY,
      windowDays: 14,
      entries,
      knownTotalMinor: entries
        .filter((entry) => entry.currency === "USD")
        .reduce((sum, entry) => sum + (entry.amountMinor ?? 0), 0),
      unknownAmountCount: entries.filter((entry) => entry.amountMinor === null).length,
      nonUsdEntryCount: entries.filter((entry) => entry.currency !== "USD").length,
      complete: entries.every((entry) => entry.amountMinor !== null),
    },
  };
}

function accountSpendFor(month: string): AccountSpend[] {
  const map = new Map<string, AccountSpend>();
  for (const tx of transactions.filter((item) => eligible(item) && inMonth(item.date, month))) {
    const key = `${tx.accountId}|${tx.currency}`;
    const item = map.get(key) ?? {
      accountId: tx.accountId,
      currency: tx.currency,
      netMinor: 0,
      count: 0,
    };
    item.netMinor += tx.amountMinor;
    item.count += 1;
    map.set(key, item);
  }
  return [...map.values()];
}

export function mockDashboard(mode: "real" | "demo", month?: string | null): Dashboard {
  const selected = month && /^\d{4}-\d{2}$/.test(month) ? month : CURRENT_MONTH;
  const bounds = monthBounds(selected);
  return {
    mode,
    demo: mode === "demo",
    month: selected,
    currentMonth: CURRENT_MONTH,
    previousMonth: shiftMonth(selected, -1),
    nextMonth: shiftMonth(selected, 1),
    startDate: bounds.start,
    endDateExclusive: bounds.end,
    currency: "USD",
    summary: summaryFor(selected),
    overview: overviewFor(),
    accountSpend: accountSpendFor(selected),
  };
}

function toRow(tx: MockTx): LedgerRow {
  const account = accounts.find((item) => item.id === tx.accountId);
  const eff = effective(tx);
  const override = overrides.get(tx.id);
  return {
    id: tx.id,
    reportingDate: tx.date,
    merchant: eff.merchant,
    merchantEdited: Boolean(override?.merchantAlias),
    rawDescription: `${tx.merchant} raw description`,
    accountId: tx.accountId,
    accountName: account?.displayName ?? "Unknown",
    accountLastFour: account?.lastFour ?? null,
    category: eff.category,
    categoryEdited: Boolean(override?.category),
    txType: eff.type,
    pending: tx.pending,
    amountMinor: tx.amountMinor,
    currency: tx.currency,
    sourceKind: "demo",
    authorizedDate: tx.authorizedDate,
    postedDate: tx.postedDate,
    reportingDateSource: tx.authorizedDate ? "authorized" : "posted",
    excluded: eff.excluded,
    notes: eff.notes,
  };
}

export function mockLedger(query: LedgerQuery): LedgerPage {
  const page = query.page ?? 0;
  const pageSize = query.pageSize ?? 50;
  let rows = transactions.filter(
    (tx) => tx.date >= query.startDate && tx.date < query.endDateExclusive,
  );
  if (query.accountIds && query.accountIds.length > 0) {
    rows = rows.filter((tx) => query.accountIds?.includes(tx.accountId));
  }
  if (query.category) {
    rows = rows.filter((tx) => (effective(tx).category ?? "Uncategorized") === query.category);
  }
  if (query.status === "pending") rows = rows.filter((tx) => tx.pending);
  if (query.status === "posted") rows = rows.filter((tx) => !tx.pending);
  if (query.txType) rows = rows.filter((tx) => effective(tx).type === query.txType);
  if (query.search) {
    const needle = query.search.toLowerCase();
    rows = rows.filter(
      (tx) =>
        effective(tx).merchant.toLowerCase().includes(needle) ||
        `${tx.merchant} raw description`.toLowerCase().includes(needle),
    );
  }
  const rowCount = rows.length;
  const subtotalByCurrency = new Map<string, CurrencySubtotal>();
  for (const tx of rows) {
    const effectiveTx = effective(tx);
    if (effectiveTx.excluded || !isSpend(effectiveTx.type)) continue;
    const entry = subtotalByCurrency.get(tx.currency) ?? {
      currency: tx.currency,
      eligibleMinor: 0,
      pendingMinor: 0,
      eligibleCount: 0,
      pendingCount: 0,
    };
    if (tx.pending) {
      entry.pendingMinor += tx.amountMinor;
      entry.pendingCount += 1;
    } else {
      entry.eligibleMinor += tx.amountMinor;
      entry.eligibleCount += 1;
    }
    subtotalByCurrency.set(tx.currency, entry);
  }
  const subtotals = [...subtotalByCurrency.values()].sort((a, b) => {
    if (a.currency === "USD") return -1;
    if (b.currency === "USD") return 1;
    return a.currency.localeCompare(b.currency);
  });
  const sorted = [...rows].sort((a, b) => {
    switch (query.sort) {
      case "date_asc":
        return a.date.localeCompare(b.date);
      case "amount_desc":
        return b.amountMinor - a.amountMinor || b.date.localeCompare(a.date);
      case "amount_asc":
        return a.amountMinor - b.amountMinor || a.date.localeCompare(b.date);
      case "merchant_asc":
        return a.merchant.localeCompare(b.merchant) || b.date.localeCompare(a.date);
      default:
        return b.date.localeCompare(a.date) || a.id.localeCompare(b.id);
    }
  });
  return {
    rows: sorted.slice(page * pageSize, page * pageSize + pageSize).map(toRow),
    rowCount,
    page,
    pageSize,
    subtotals,
  };
}

// ---------- Phase 6 mock editors/imports (browser tests only) ----------

interface MockOverride {
  merchantAlias: string | null;
  category: string | null;
  typeCorrection:
    | "purchase"
    | "refund"
    | "fee"
    | "interest"
    | "payment"
    | "transfer"
    | "other"
    | null;
  excluded: boolean | null;
  notes: string | null;
}

const overrides = new Map<string, MockOverride>();
const merchantRules: import("../lib/bridge/contract").MerchantRule[] = [];
const manualPayments = new Map<
  string,
  {
    statementBalanceMinor: number | null;
    minimumDueMinor: number | null;
    nextDueDate: string | null;
    validThroughDate: string | null;
    updatedAt: string;
  }
>();
const importBatches: import("../lib/bridge/contract").ImportBatch[] = [];
let importJob: { jobId: string; accountId: string; consumed: boolean } | null = null;

function toMinor(value: string | null): number | null {
  if (!value || value.trim() === "") return null;
  const normalized = value.replace(/[$,]/g, "").trim();
  if (!/^\d+(\.\d{1,2})?$/.test(normalized)) {
    throw new BridgeError("invalid_input", "Invalid amount.");
  }
  return Math.round(Number(normalized) * 100);
}

function effective(tx: MockTx) {
  const override = overrides.get(tx.id);
  return {
    merchant: override?.merchantAlias ?? tx.merchant,
    category: override?.category ?? tx.category,
    type: override?.typeCorrection ?? tx.txType,
    excluded: override?.excluded ?? tx.excluded,
    notes: override?.notes ?? null,
  };
}

export function mockTransactionDetail(id: string) {
  const tx = transactions.find((item) => item.id === id);
  if (!tx) throw new BridgeError("not_found", "Transaction not found.");
  const override = overrides.get(id);
  const account = accounts.find((item) => item.id === tx.accountId);
  return {
    id: tx.id,
    accountId: tx.accountId,
    accountName: account?.displayName ?? "Unknown",
    accountLastFour: account?.lastFour ?? null,
    reportingDate: tx.date,
    authorizedDate: tx.authorizedDate,
    postedDate: tx.postedDate,
    reportingDateSource: tx.authorizedDate ? "authorized" : "posted",
    amountMinor: tx.amountMinor,
    currency: tx.currency,
    rawDescription: `${tx.merchant} raw description`,
    merchant: override?.merchantAlias ?? tx.merchant,
    merchantAlias: override?.merchantAlias ?? null,
    category: override?.category ?? tx.category,
    sourceCategory: tx.category,
    txType: override?.typeCorrection ?? tx.txType,
    typeCorrection: override?.typeCorrection ?? null,
    pending: tx.pending,
    excluded: override?.excluded ?? tx.excluded,
    excludedOverride: override?.excluded ?? null,
    notes: override?.notes ?? null,
    sourceKind: "demo",
    providerTransactionId: null,
    pendingLinkId: null,
    editedAt: override ? "2026-10-23T12:00:00Z" : null,
  };
}

export function mockSetOverride(
  id: string,
  input: {
    merchantAlias: string | null;
    category: string | null;
    typeCorrection: MockOverride["typeCorrection"];
    excluded: boolean | null;
    notes: string | null;
  },
) {
  if (!transactions.some((tx) => tx.id === id)) {
    throw new BridgeError("not_found", "Transaction not found.");
  }
  const empty =
    input.merchantAlias === null &&
    input.category === null &&
    input.typeCorrection === null &&
    input.excluded === null &&
    input.notes === null;
  if (empty) overrides.delete(id);
  else overrides.set(id, input);
  return mockTransactionDetail(id);
}

export function mockMerchantRules() {
  return structuredClone(merchantRules);
}

export function mockCreateMerchantRule(input: {
  matchType: "exact" | "contains" | "starts_with";
  matchText: string;
  canonicalMerchant: string;
  category: string | null;
  priority: number;
  enabled: boolean;
}) {
  const rule: import("../lib/bridge/contract").MerchantRule = {
    id: crypto.randomUUID(),
    matchType: input.matchType,
    matchText: input.matchText.toLowerCase(),
    canonicalMerchant: input.canonicalMerchant,
    category: input.category,
    priority: input.priority,
    enabled: input.enabled,
    createdAt: "2026-10-23T12:00:00Z",
    updatedAt: "2026-10-23T12:00:00Z",
  };
  merchantRules.push(rule);
  return structuredClone(rule);
}

export function mockUpdateMerchantRule(
  id: string,
  input: Parameters<typeof mockCreateMerchantRule>[0],
) {
  const index = merchantRules.findIndex((rule) => rule.id === id);
  if (index < 0) throw new BridgeError("not_found", "Rule not found.");
  const updated = {
    ...merchantRules[index],
    matchType: input.matchType,
    matchText: input.matchText.toLowerCase(),
    canonicalMerchant: input.canonicalMerchant,
    category: input.category,
    priority: input.priority,
    enabled: input.enabled,
    updatedAt: "2026-10-23T12:00:00Z",
  };
  merchantRules[index] = updated;
  return structuredClone(updated);
}

export function mockDeleteMerchantRule(id: string) {
  const index = merchantRules.findIndex((rule) => rule.id === id);
  if (index < 0) throw new BridgeError("not_found", "Rule not found.");
  merchantRules.splice(index, 1);
}

export function mockManualPayment(accountId: string) {
  const value = manualPayments.get(accountId);
  if (!value) return null;
  return {
    accountId,
    statementBalanceMinor: value.statementBalanceMinor,
    minimumDueMinor: value.minimumDueMinor,
    nextDueDate: value.nextDueDate,
    validThroughDate: value.validThroughDate,
    createdAt: "2026-10-23T12:00:00Z",
    updatedAt: value.updatedAt,
  };
}

export function mockSetManualPayment(
  accountId: string,
  input: {
    statementBalance: string | null;
    minimumDue: string | null;
    nextDueDate: string | null;
    validThroughDate: string | null;
  },
) {
  const statementBalanceMinor = toMinor(input.statementBalance);
  const minimumDueMinor = toMinor(input.minimumDue);
  if (statementBalanceMinor === null && minimumDueMinor === null && !input.nextDueDate) {
    manualPayments.delete(accountId);
    return null;
  }
  manualPayments.set(accountId, {
    statementBalanceMinor,
    minimumDueMinor,
    nextDueDate: input.nextDueDate,
    validThroughDate: input.validThroughDate,
    updatedAt: "2026-10-23T12:00:00Z",
  });
  return mockManualPayment(accountId);
}

export function mockChooseImportFile(accountId: string) {
  if (!accounts.some((account) => account.id === accountId)) {
    throw new BridgeError("not_found", "Account not found.");
  }
  importJob = { jobId: "mock-import-job", accountId, consumed: false };
  return {
    jobId: importJob.jobId,
    headers: ["Date", "Description", "Amount", "Category", "Source ID"],
    sizeBytes: 2048,
    rowCount: 3,
  };
}

export function mockValidateImport(jobId: string) {
  if (!importJob || importJob.jobId !== jobId) throw new BridgeError("not_found", "Import job.");
  return {
    jobId,
    rowCount: 3,
    insertCount: 2,
    duplicateCount: 0,
    reviewCount: 1,
    rejectedCount: 0,
    sampleRows: [
      {
        rowNumber: 1,
        date: "2026-10-02",
        merchant: "Imported Coffee",
        amountMinor: 1234,
        category: "Dining",
        status: "insert",
        reason: null,
      },
      {
        rowNumber: 2,
        date: "2026-10-03",
        merchant: "Imported Books",
        amountMinor: 4500,
        category: "Shopping",
        status: "insert",
        reason: null,
      },
      {
        rowNumber: 3,
        date: "2026-10-04",
        merchant: "Imported Coffee",
        amountMinor: 1234,
        category: "Dining",
        status: "review",
        reason: "possible duplicate; review before importing",
      },
    ],
    months: [{ month: "2026-10", purchasesMinor: 5734, refundsMinor: 0, netMinor: 5734, count: 2 }],
    reviewMonths: [
      { month: "2026-10", purchasesMinor: 1234, refundsMinor: 0, netMinor: 1234, count: 1 },
    ],
    noOp: false,
    existingBatchId: null,
  };
}

export function mockCommitImport(jobId: string, acceptReview: boolean) {
  if (!importJob || importJob.jobId !== jobId) throw new BridgeError("not_found", "Import job.");
  const batch: import("../lib/bridge/contract").ImportBatch = {
    id: crypto.randomUUID(),
    accountId: importJob.accountId,
    accountName: accounts.find((a) => a.id === importJob?.accountId)?.displayName ?? "Account",
    fileSha256: "0".repeat(64),
    rowCount: 3,
    insertedCount: acceptReview ? 3 : 2,
    duplicateCount: 0,
    reviewCount: acceptReview ? 0 : 1,
    rejectedCount: 0,
    status: "imported",
    createdAt: "2026-10-23T12:00:00Z",
    importedAt: "2026-10-23T12:00:00Z",
    rolledBackAt: null,
  };
  importBatches.unshift(batch);
  transactions = [
    ...transactions,
    makeTx(
      `mock-import-${batch.id}-1`,
      importJob.accountId,
      "2026-10-02",
      1234,
      "purchase",
      "Imported Coffee",
      "Dining",
    ),
    makeTx(
      `mock-import-${batch.id}-2`,
      importJob.accountId,
      "2026-10-03",
      4500,
      "purchase",
      "Imported Books",
      "Shopping",
    ),
  ];
  importJob = null;
  return {
    batchId: batch.id,
    inserted: batch.insertedCount,
    duplicates: 0,
    review: batch.reviewCount,
    rejected: 0,
    months: [{ month: "2026-10", purchasesMinor: 5734, refundsMinor: 0, netMinor: 5734, count: 2 }],
    noOp: false,
  };
}

export function mockImportBatches() {
  return structuredClone(importBatches);
}

export function mockRollbackImport(batchId: string) {
  const batch = importBatches.find((item) => item.id === batchId);
  if (!batch) throw new BridgeError("not_found", "Batch not found.");
  batch.status = "rolled_back";
  batch.rolledBackAt = "2026-10-23T12:00:00Z";
  transactions = transactions.filter((tx) => !tx.id.startsWith(`mock-import-${batchId}`));
  return structuredClone(batch);
}

export function mockExportCount(query: LedgerQuery): number {
  return mockLedger(query).rowCount;
}

resetMockData();
