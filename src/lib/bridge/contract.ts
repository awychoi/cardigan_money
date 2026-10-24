import { z } from "zod";

/**
 * IPC contract, mirrored from Rust (`src-tauri/src/commands`).
 * The Rust gateway is authoritative. These schemas validate responses at the
 * renderer boundary so a contract drift fails loudly instead of silently
 * becoming `undefined` in the UI.
 *
 * Keep `AppInfo` and `VaultStatus` in sync with `fixtures/contract/*.json`,
 * which both test suites parse.
 */

export const API_VERSION = 1;
export const MAX_REQUEST_BYTES = 256 * 1024;

export const zEmpty = z.object({}).strict();

export const zAppInfo = z
  .object({
    name: z.literal("cardigan_money"),
    version: z.string().min(1).max(32),
    bundleIdentifier: z.literal("com.awychoi.cardiganmoney"),
    platform: z.string().min(1).max(32),
    apiVersion: z.literal(1),
    vaultExists: z.boolean(),
    vaultUnlocked: z.boolean(),
    demoAvailable: z.boolean(),
  })
  .strict();

export const zVaultStatus = z
  .object({
    exists: z.boolean(),
    unlocked: z.boolean(),
    mode: z.enum(["real", "demo"]),
  })
  .strict();

export const zHealthCheck = z
  .object({
    ok: z.literal(true),
    apiVersion: z.literal(1),
  })
  .strict();


export const zTheme = z.enum(["light", "dark", "system"]);
export const zCloseBehavior = z.enum(["unset", "hide", "quit"]);

export const zSettings = z
  .object({
    timezone: z.string().min(1).max(64),
    theme: zTheme,
    closeBehavior: zCloseBehavior,
    onboardingCompleted: z.boolean(),
    autoLockMinutes: z.number().int().min(1).max(240),
    lockOnSystemEvents: z.boolean(),
    contentProtection: z.boolean(),
    syncWindows: z.array(z.string().regex(/^\d{2}:\d{2}$/)).max(8),
    syncOnLaunch: z.boolean(),
    backupRetentionDaily: z.number().int().min(1).max(30),
    lastBackupAt: z.string().nullable(),
    lastBackupStatus: z.string().nullable(),
  })
  .strict();

export const zSecuritySummary = z
  .object({
    cipherVersion: z.string(),
    cipherProvider: z.string(),
    kdfSummary: z.string(),
    generation: z.number().int().positive(),
    schemaVersion: z.number().int().positive(),
    lastIntegrityCheckAt: z.string().nullable(),
    vaultIdPrefix: z.string(),
    cipherStatusSupported: z.boolean(),
  })
  .strict();

export const zVaultState = z
  .object({
    exists: z.boolean(),
    unlocked: z.boolean(),
    mode: z.enum(["real", "demo"]),
    settings: zSettings.nullable(),
    security: zSecuritySummary.nullable(),
  })
  .strict();

export const zSecurityEvent = z
  .object({
    id: z.string(),
    eventType: z.string(),
    outcome: z.enum(["success", "failure", "info"]),
    createdAt: z.string(),
    metadata: z.record(z.string(), z.unknown()).nullable(),
  })
  .strict();

export const zSettingsView = z.object({ settings: zSettings }).strict();
export const zSecurityView = z.object({ security: zSecuritySummary }).strict();
export const zEventsView = z.object({ events: z.array(zSecurityEvent) }).strict();

export const zSettingsPatch = z
  .object({
    timezone: z.string().min(1).max(64).optional(),
    theme: zTheme.optional(),
    closeBehavior: zCloseBehavior.optional(),
    onboardingCompleted: z.boolean().optional(),
    autoLockMinutes: z.number().int().min(1).max(240).optional(),
    syncWindows: z.array(z.string().regex(/^\d{2}:\d{2}$/)).max(8).optional(),
    syncOnLaunch: z.boolean().optional(),
    backupRetentionDaily: z.number().int().min(1).max(30).optional(),
  })
  .strict();

export const zEventsRequest = z
  .object({ limit: z.number().int().min(1).max(100).optional() })
  .strict();


export const zAccountType = z.enum(["credit_card", "charge_card", "line_of_credit", "other"]);

export const zAccountPayload = z
  .object({
    issuer: z.string().trim().min(1).max(80),
    displayName: z.string().trim().min(1).max(120),
    lastFour: z
      .string()
      .trim()
      .regex(/^\d{4}$/)
      .nullable(),
    accountType: zAccountType,
    currency: z.string().regex(/^[A-Z]{3}$/),
    creditLimit: z.string().trim().max(40).nullable(),
    includeInOverview: z.boolean(),
  })
  .strict();

export const zAccount = z
  .object({
    id: z.string(),
    issuer: z.string(),
    displayName: z.string(),
    lastFour: z.string().nullable(),
    accountType: z.string(),
    currency: z.string(),
    creditLimitMinor: z.number().int().nullable(),
    includeInOverview: z.boolean(),
    archived: z.boolean(),
    sourceKind: z.enum(["manual", "provider", "demo"]),
    createdAt: z.string(),
    updatedAt: z.string(),
  })
  .strict();

export const zAccountsView = z.object({ accounts: z.array(zAccount) }).strict();
export const zAccountsListRequest = z
  .object({ includeArchived: z.boolean().optional() })
  .strict();
export const zAccountUpdateRequest = z
  .object({ id: z.string().min(1).max(64), account: zAccountPayload })
  .strict();
export const zAccountArchiveRequest = z
  .object({ id: z.string().min(1).max(64), archived: z.boolean() })
  .strict();

export type Account = z.infer<typeof zAccount>;
export type AccountPayload = z.infer<typeof zAccountPayload>;


export const zPaymentInfo = z
  .object({
    nextDueDate: z.string().nullable(),
    dueAmountMinor: z.number().int().nullable(),
    dueAmountKind: z.enum(["minimum_due", "reported_zero", "not_reported", "no_due_date"]),
    dueSource: z.string().nullable(),
    statementBalanceMinor: z.number().int().nullable(),
    statementBalanceSource: z.string().nullable(),
    statementIssueDate: z.string().nullable(),
    nextClosingDate: z.string().nullable(),
    validThroughDate: z.string().nullable(),
    manualExpired: z.boolean(),
    retrievedAt: z.string().nullable(),
  })
  .strict();

export const zAccountCard = z
  .object({
    accountId: z.string(),
    issuer: z.string(),
    displayName: z.string(),
    lastFour: z.string().nullable(),
    currency: z.string(),
    included: z.boolean(),
    sourceKind: z.string(),
    connectionStatus: z.string().nullable(),
    currentBalanceMinor: z.number().int().nullable(),
    availableCreditMinor: z.number().int().nullable(),
    creditLimitMinor: z.number().int().nullable(),
    balanceRetrievedAt: z.string().nullable(),
    balanceProvenance: z.string().nullable(),
    balanceAgeDays: z.number().int().nullable(),
    balanceStale: z.boolean(),
    payment: zPaymentInfo,
  })
  .strict();

export const zBalanceTotals = z
  .object({
    knownTotalMinor: z.number().int(),
    accountsWithBalance: z.number().int(),
    accountsMissingBalance: z.number().int(),
    complete: z.boolean(),
    otherCurrency: z.array(z.tuple([z.string(), z.number().int()])),
  })
  .strict();

export const zUpcomingPayment = z
  .object({
    accountId: z.string(),
    displayName: z.string(),
    lastFour: z.string().nullable(),
    currency: z.string(),
    dueDate: z.string(),
    amountMinor: z.number().int().nullable(),
    amountKind: z.string(),
    source: z.string(),
  })
  .strict();

export const zUpcoming = z
  .object({
    asOfDate: z.string(),
    windowDays: z.number().int(),
    entries: z.array(zUpcomingPayment),
    knownTotalMinor: z.number().int(),
    unknownAmountCount: z.number().int(),
    nonUsdEntryCount: z.number().int(),
    complete: z.boolean(),
  })
  .strict();

export const zOverview = z
  .object({
    accounts: z.array(zAccountCard),
    balances: zBalanceTotals,
    upcoming: zUpcoming,
  })
  .strict();

export const zBreakdown = z
  .object({
    key: z.string(),
    grossMinor: z.number().int(),
    refundsMinor: z.number().int(),
    netMinor: z.number().int(),
    count: z.number().int(),
  })
  .strict();

export const zReconciliation = z
  .object({
    eligibleNetMinor: z.number().int(),
    supplierNetMinor: z.number().int(),
    categoryNetMinor: z.number().int(),
    exact: z.boolean(),
  })
  .strict();

export const zMonthSummary = z
  .object({
    month: z.string(),
    startDate: z.string(),
    endDateExclusive: z.string(),
    currency: z.string(),
    grossPurchasesMinor: z.number().int(),
    refundsMinor: z.number().int(),
    feesInterestMinor: z.number().int(),
    netSpendingMinor: z.number().int(),
    pendingSubtotalMinor: z.number().int(),
    eligibleCount: z.number().int(),
    pendingCount: z.number().int(),
    rowCount: z.number().int(),
    otherCurrencyRowCount: z.number().int(),
    suppliers: z.array(zBreakdown),
    categories: z.array(zBreakdown),
    reconciliation: zReconciliation,
  })
  .strict();

export const zAccountSpend = z
  .object({
    accountId: z.string(),
    currency: z.string(),
    netMinor: z.number().int(),
    count: z.number().int(),
  })
  .strict();

export const zDashboard = z
  .object({
    mode: z.enum(["real", "demo"]),
    demo: z.boolean(),
    month: z.string(),
    currentMonth: z.string(),
    previousMonth: z.string(),
    nextMonth: z.string(),
    startDate: z.string(),
    endDateExclusive: z.string(),
    currency: z.string(),
    summary: zMonthSummary,
    overview: zOverview,
    accountSpend: z.array(zAccountSpend),
  })
  .strict();

export const zLedgerRow = z
  .object({
    id: z.string(),
    reportingDate: z.string(),
    merchant: z.string(),
    merchantEdited: z.boolean(),
    rawDescription: z.string(),
    accountId: z.string(),
    accountName: z.string(),
    accountLastFour: z.string().nullable(),
    category: z.string().nullable(),
    categoryEdited: z.boolean(),
    txType: z.string(),
    pending: z.boolean(),
    amountMinor: z.number().int(),
    currency: z.string(),
    sourceKind: z.string(),
    authorizedDate: z.string().nullable(),
    postedDate: z.string().nullable(),
    reportingDateSource: z.string(),
    excluded: z.boolean(),
    notes: z.string().nullable(),
  })
  .strict();

export const zLedgerPage = z
  .object({
    rows: z.array(zLedgerRow),
    rowCount: z.number().int(),
    page: z.number().int(),
    pageSize: z.number().int(),
    eligibleSubtotalMinor: z.number().int(),
    pendingSubtotalMinor: z.number().int(),
  })
  .strict();

export const zLedgerQuery = z
  .object({
    startDate: z.string().regex(/^\d{4}-\d{2}-\d{2}$/),
    endDateExclusive: z.string().regex(/^\d{4}-\d{2}-\d{2}$/),
    accountIds: z.array(z.string()).max(20).optional(),
    category: z.string().max(80).nullable().optional(),
    status: z.enum(["all", "pending", "posted"]).optional(),
    txType: z
      .enum(["purchase", "refund", "fee", "interest", "payment", "transfer", "other"])
      .optional(),
    search: z.string().max(120).nullable().optional(),
    sort: z.enum(["date_desc", "date_asc", "amount_desc", "amount_asc", "merchant_asc"]).optional(),
    page: z.number().int().min(0).max(10000).optional(),
    pageSize: z.number().int().min(1).max(200).optional(),
  })
  .strict();

export type PaymentInfo = z.infer<typeof zPaymentInfo>;
export type UpcomingPayment = z.infer<typeof zUpcomingPayment>;
export type AccountCard = z.infer<typeof zAccountCard>;
export type Overview = z.infer<typeof zOverview>;
export type MonthSummary = z.infer<typeof zMonthSummary>;
export type Breakdown = z.infer<typeof zBreakdown>;
export type AccountSpend = z.infer<typeof zAccountSpend>;
export type Dashboard = z.infer<typeof zDashboard>;
export type LedgerRow = z.infer<typeof zLedgerRow>;
export type LedgerPage = z.infer<typeof zLedgerPage>;
export type LedgerQuery = z.infer<typeof zLedgerQuery>;


export const zTransactionDetail = z
  .object({
    id: z.string(),
    accountId: z.string(),
    accountName: z.string(),
    accountLastFour: z.string().nullable(),
    reportingDate: z.string(),
    authorizedDate: z.string().nullable(),
    postedDate: z.string().nullable(),
    reportingDateSource: z.string(),
    amountMinor: z.number().int(),
    currency: z.string(),
    rawDescription: z.string(),
    merchant: z.string(),
    merchantAlias: z.string().nullable(),
    category: z.string().nullable(),
    sourceCategory: z.string().nullable(),
    txType: z.string(),
    typeCorrection: z.string().nullable(),
    pending: z.boolean(),
    excluded: z.boolean(),
    excludedOverride: z.boolean().nullable(),
    notes: z.string().nullable(),
    sourceKind: z.string(),
    providerTransactionId: z.string().nullable(),
    pendingLinkId: z.string().nullable(),
    editedAt: z.string().nullable(),
  })
  .strict();

export const zOverrideInput = z
  .object({
    merchantAlias: z.string().max(120).nullable(),
    category: z.string().max(80).nullable(),
    typeCorrection: z
      .enum(["purchase", "refund", "fee", "interest", "payment", "transfer", "other"])
      .nullable(),
    excluded: z.boolean().nullable(),
    notes: z.string().max(2000).nullable(),
  })
  .strict();

export const zMerchantRuleInput = z
  .object({
    matchType: z.enum(["exact", "contains", "starts_with"]),
    matchText: z.string().trim().min(1).max(120),
    canonicalMerchant: z.string().trim().min(1).max(120),
    category: z.string().max(80).nullable(),
    priority: z.number().int().min(0).max(10000),
    enabled: z.boolean(),
  })
  .strict();

export const zMerchantRule = z
  .object({
    id: z.string(),
    matchType: z.string(),
    matchText: z.string(),
    canonicalMerchant: z.string(),
    category: z.string().nullable(),
    priority: z.number().int(),
    enabled: z.boolean(),
    createdAt: z.string(),
    updatedAt: z.string(),
  })
  .strict();

export const zManualPaymentInput = z
  .object({
    statementBalance: z.string().max(40).nullable(),
    minimumDue: z.string().max(40).nullable(),
    nextDueDate: z.string().max(10).nullable(),
    validThroughDate: z.string().max(10).nullable(),
  })
  .strict();

export const zManualPayment = z
  .object({
    accountId: z.string(),
    statementBalanceMinor: z.number().int().nullable(),
    minimumDueMinor: z.number().int().nullable(),
    nextDueDate: z.string().nullable(),
    validThroughDate: z.string().nullable(),
    createdAt: z.string(),
    updatedAt: z.string(),
  })
  .strict();

export const zCsvMapping = z
  .object({
    dateColumn: z.number().int().min(0).max(39),
    descriptionColumn: z.number().int().min(0).max(39),
    amountColumn: z.number().int().min(0).max(39).nullable(),
    debitColumn: z.number().int().min(0).max(39).nullable(),
    creditColumn: z.number().int().min(0).max(39).nullable(),
    categoryColumn: z.number().int().min(0).max(39).nullable(),
    sourceIdColumn: z.number().int().min(0).max(39).nullable(),
    dateFormat: z.enum(["iso", "us", "eu"]),
    signConvention: z.enum(["signed_amount", "debit_credit"]),
    creditType: z.enum(["refund", "payment", "other"]),
  })
  .strict();

export const zPreviewRow = z
  .object({
    rowNumber: z.number().int(),
    date: z.string(),
    merchant: z.string(),
    amountMinor: z.number().int(),
    category: z.string().nullable(),
    status: z.string(),
    reason: z.string().nullable(),
  })
  .strict();

export const zMonthPreview = z
  .object({
    month: z.string(),
    purchasesMinor: z.number().int(),
    refundsMinor: z.number().int(),
    netMinor: z.number().int(),
    count: z.number().int(),
  })
  .strict();

export const zImportFileView = z
  .object({
    jobId: z.string(),
    headers: z.array(z.string()).max(40),
    sizeBytes: z.number().int(),
    rowCount: z.number().int(),
  })
  .strict();

export const zValidationPreview = z
  .object({
    jobId: z.string(),
    rowCount: z.number().int(),
    insertCount: z.number().int(),
    duplicateCount: z.number().int(),
    reviewCount: z.number().int(),
    rejectedCount: z.number().int(),
    sampleRows: z.array(zPreviewRow),
    months: z.array(zMonthPreview),
    reviewMonths: z.array(zMonthPreview),
    noOp: z.boolean(),
    existingBatchId: z.string().nullable(),
  })
  .strict();

export const zImportResult = z
  .object({
    batchId: z.string(),
    inserted: z.number().int(),
    duplicates: z.number().int(),
    review: z.number().int(),
    rejected: z.number().int(),
    months: z.array(zMonthPreview),
    noOp: z.boolean(),
  })
  .strict();

export const zImportBatch = z
  .object({
    id: z.string(),
    accountId: z.string(),
    accountName: z.string(),
    fileSha256: z.string(),
    rowCount: z.number().int(),
    insertedCount: z.number().int(),
    duplicateCount: z.number().int(),
    reviewCount: z.number().int(),
    rejectedCount: z.number().int(),
    status: z.string(),
    createdAt: z.string(),
    importedAt: z.string().nullable(),
    rolledBackAt: z.string().nullable(),
  })
  .strict();

export const zExportResult = z.object({ rows: z.number().int(), written: z.boolean() }).strict();

export type TransactionDetail = z.infer<typeof zTransactionDetail>;
export type OverrideInput = z.infer<typeof zOverrideInput>;
export type MerchantRule = z.infer<typeof zMerchantRule>;
export type MerchantRuleInput = z.infer<typeof zMerchantRuleInput>;
export type ManualPayment = z.infer<typeof zManualPayment>;
export type ManualPaymentInput = z.infer<typeof zManualPaymentInput>;
export type CsvMapping = z.infer<typeof zCsvMapping>;
export type ImportFileView = z.infer<typeof zImportFileView>;
export type ValidationPreview = z.infer<typeof zValidationPreview>;
export type ImportResult = z.infer<typeof zImportResult>;
export type ImportBatch = z.infer<typeof zImportBatch>;
export type ExportResult = z.infer<typeof zExportResult>;


export const zBackupItem = z
  .object({
    id: z.string(),
    kind: z.string(),
    createdAt: z.string(),
    sizeBytes: z.number().int(),
    generation: z.number().int(),
    schemaVersion: z.number().int(),
  })
  .strict();
export const zBackupResult = z
  .object({ backup: zBackupItem, pruned: z.number().int() })
  .strict();
export const zRestoreResult = z
  .object({
    restored: z.boolean(),
    kind: z.string(),
    preRestoreBackupId: z.string(),
  })
  .strict();

export type BackupItem = z.infer<typeof zBackupItem>;
export type BackupResult = z.infer<typeof zBackupResult>;
export type RestoreResult = z.infer<typeof zRestoreResult>;


export const zConnectionView = z
  .object({
    id: z.string(),
    provider: z.string(),
    environment: z.string(),
    institutionName: z.string().nullable(),
    status: z.string(),
    lastAttemptAt: z.string().nullable(),
    lastSuccessAt: z.string().nullable(),
    sanitizedError: z.string().nullable(),
    hasCursor: z.boolean(),
    accountCount: z.number().int(),
  })
  .strict();
export const zSyncStepView = z
  .object({
    stream: z.string(),
    status: z.string(),
    itemsAdded: z.number().int(),
    itemsModified: z.number().int(),
    itemsRemoved: z.number().int(),
    sanitizedError: z.string().nullable(),
  })
  .strict();
export const zSyncRunView = z
  .object({
    id: z.string(),
    connectionId: z.string(),
    institutionName: z.string().nullable(),
    trigger: z.string(),
    status: z.string(),
    startedAt: z.string(),
    finishedAt: z.string().nullable(),
    errorCode: z.string().nullable(),
    steps: z.array(zSyncStepView),
  })
  .strict();
export const zSyncRuntime = z
  .object({
    running: z.boolean(),
    lastResult: z.string().nullable(),
    lastSkipReason: z.string().nullable(),
    lastAttemptAt: z.string().nullable(),
    lastFinishedAt: z.string().nullable(),
  })
  .strict();
export const zSyncStatus = z
  .object({
    connections: z.array(zConnectionView),
    recentRuns: z.array(zSyncRunView),
    runtime: zSyncRuntime,
    providerLive: z.boolean(),
  })
  .strict();
export const zSyncSummary = z
  .object({
    accepted: z.boolean(),
    skipReason: z.string().nullable(),
    results: z.array(
      z
        .object({
          connectionId: z.string(),
          institution: z.string(),
          status: z.string(),
          transactionsAdded: z.number().int(),
          transactionsModified: z.number().int(),
          transactionsRemoved: z.number().int(),
          balancesWritten: z.number().int(),
          liabilitiesStatus: z.string(),
          errorCode: z.string().nullable(),
        })
        .strict(),
    ),
    startedAt: z.string(),
    finishedAt: z.string(),
  })
  .strict();

export type ConnectionView = z.infer<typeof zConnectionView>;
export type SyncRunView = z.infer<typeof zSyncRunView>;
export type SyncRuntime = z.infer<typeof zSyncRuntime>;
export type SyncStatus = z.infer<typeof zSyncStatus>;
export type SyncSummary = z.infer<typeof zSyncSummary>;


export const zPlaidStatus = z
  .object({
    configured: z.boolean(),
    environment: z.string(),
    linkAvailable: z.boolean(),
    message: z.string().nullable(),
  })
  .strict();
export const zPlaidLinkStatus = z
  .object({
    state: z.string(),
    message: z.string().nullable(),
    connectionId: z.string().nullable(),
    expiresAt: z.string().nullable(),
  })
  .strict();
export type PlaidStatus = z.infer<typeof zPlaidStatus>;
export type PlaidLinkStatus = z.infer<typeof zPlaidLinkStatus>;


export const zPlaidVerificationReport = z
  .object({
    environment: z.string(),
    connections: z.array(
      z
        .object({
          institutionName: z.string().nullable(),
          environment: z.string(),
          status: z.string(),
          accountCount: z.number().int(),
          providerTransactionCount: z.number().int(),
          pendingTransactionCount: z.number().int(),
          lastSuccessAt: z.string().nullable(),
          cursorPresent: z.boolean(),
        })
        .strict(),
    ),
    latestRunStatus: z.string().nullable(),
    latestLiabilitiesStatus: z.string().nullable(),
    recentRuns: z.array(zSyncRunView),
    generatedAt: z.string(),
  })
  .strict();
export type PlaidVerificationReport = z.infer<typeof zPlaidVerificationReport>;

export type Settings = z.infer<typeof zSettings>;
export type SettingsPatch = z.infer<typeof zSettingsPatch>;
export type SecuritySummary = z.infer<typeof zSecuritySummary>;
export type VaultState = z.infer<typeof zVaultState>;
export type SecurityEvent = z.infer<typeof zSecurityEvent>;
export type ThemeChoice = z.infer<typeof zTheme>;

export type AppInfo = z.infer<typeof zAppInfo>;
export type VaultStatus = z.infer<typeof zVaultStatus>;
export type HealthCheck = z.infer<typeof zHealthCheck>;

export interface CommandDefs {
  app_info: { request: Record<string, never>; response: AppInfo };
  health_check: { request: Record<string, never>; response: HealthCheck };
  vault_status: { request: Record<string, never>; response: VaultState };
  vault_create: { request: Record<string, never>; response: VaultState };
  vault_unlock: { request: Record<string, never>; response: VaultState };
  vault_lock: { request: Record<string, never>; response: VaultState };
  vault_change_passphrase: { request: Record<string, never>; response: VaultState };
  settings_get: { request: Record<string, never>; response: { settings: Settings } };
  settings_update: { request: SettingsPatch; response: { settings: Settings } };
  security_status: { request: Record<string, never>; response: { security: SecuritySummary } };
  security_events_list: { request: { limit?: number }; response: { events: SecurityEvent[] } };
  accounts_list: { request: { includeArchived?: boolean }; response: { accounts: Account[] } };
  account_create: { request: AccountPayload; response: Account };
  account_update: { request: { id: string; account: AccountPayload }; response: Account };
  account_archive: { request: { id: string; archived: boolean }; response: Account };
  app_quit: { request: Record<string, never>; response: { quitting: boolean } };
  demo_open: { request: Record<string, never>; response: VaultState };
  demo_close: { request: Record<string, never>; response: VaultState };
  demo_reset: { request: Record<string, never>; response: VaultState };
  dashboard_get: { request: { month?: string | null }; response: Dashboard };
  ledger_query: { request: LedgerQuery; response: LedgerPage };
  transaction_detail: { request: { id: string }; response: TransactionDetail };
  transaction_override_set: {
    request: { id: string; override: OverrideInput };
    response: TransactionDetail;
  };
  transaction_override_clear: { request: { id: string }; response: TransactionDetail };
  merchant_rules_list: { request: Record<string, never>; response: { rules: MerchantRule[] } };
  merchant_rule_create: { request: MerchantRuleInput; response: MerchantRule };
  merchant_rule_update: {
    request: { id: string; rule: MerchantRuleInput };
    response: MerchantRule;
  };
  merchant_rule_delete: { request: { id: string }; response: { deleted: boolean } };
  manual_payment_get: {
    request: { accountId: string };
    response: { payment: ManualPayment | null };
  };
  manual_payment_set: {
    request: { accountId: string; payment: ManualPaymentInput };
    response: { payment: ManualPayment | null };
  };
  manual_payment_clear: {
    request: { accountId: string };
    response: { payment: ManualPayment | null };
  };
  import_choose_file: { request: { accountId: string }; response: ImportFileView };
  import_validate: {
    request: { jobId: string; mapping: CsvMapping };
    response: ValidationPreview;
  };
  import_commit: {
    request: { jobId: string; mapping: CsvMapping; acceptReview: boolean };
    response: ImportResult;
  };
  import_cancel: { request: { jobId: string }; response: { cancelled: boolean } };
  import_batches_list: {
    request: { limit?: number };
    response: { batches: ImportBatch[] };
  };
  import_rollback: { request: { batchId: string }; response: ImportBatch };
  export_ledger_csv: { request: { query: LedgerQuery }; response: ExportResult };
  backup_create: { request: Record<string, never>; response: BackupResult };
  backup_list: { request: Record<string, never>; response: { backups: BackupItem[] } };
  backup_delete: { request: { id: string }; response: { deleted: boolean } };
  backup_restore: { request: Record<string, never>; response: RestoreResult };
  portable_export: { request: Record<string, never>; response: BackupItem };
  crypto_erase: { request: Record<string, never>; response: VaultState };
  sync_status: { request: Record<string, never>; response: SyncStatus };
  sync_now: { request: Record<string, never>; response: SyncSummary };
  sync_runs_list: { request: { limit?: number }; response: { runs: SyncRunView[] } };
  connection_disconnect: {
    request: { connectionId: string };
    response: { deleted: boolean };
  };
  plaid_status: { request: Record<string, never>; response: { plaid: PlaidStatus } };
  plaid_setup: { request: Record<string, never>; response: { plaid: PlaidStatus } };
  plaid_test: { request: Record<string, never>; response: { plaid: PlaidStatus } };
  plaid_link_start: { request: Record<string, never>; response: { link: PlaidLinkStatus } };
  plaid_link_status: { request: Record<string, never>; response: { link: PlaidLinkStatus } };
  plaid_link_cancel: { request: Record<string, never>; response: { link: PlaidLinkStatus } };
  plaid_reconnect: {
    request: { connectionId: string };
    response: { link: PlaidLinkStatus };
  };
  plaid_verification_report: {
    request: Record<string, never>;
    response: PlaidVerificationReport;
  };
}

export type CommandName = keyof CommandDefs;
export type CommandRequest<K extends CommandName> = CommandDefs[K]["request"];
export type CommandResponse<K extends CommandName> = CommandDefs[K]["response"];

export const requestSchemas: { [K in CommandName]: z.ZodType<CommandDefs[K]["request"]> } = {
  app_info: zEmpty,
  health_check: zEmpty,
  vault_status: zEmpty,
  vault_create: zEmpty,
  vault_unlock: zEmpty,
  vault_lock: zEmpty,
  vault_change_passphrase: zEmpty,
  settings_get: zEmpty,
  settings_update: zSettingsPatch,
  security_status: zEmpty,
  security_events_list: zEventsRequest,
  accounts_list: zAccountsListRequest,
  account_create: zAccountPayload,
  account_update: zAccountUpdateRequest,
  account_archive: zAccountArchiveRequest,
  app_quit: zEmpty,
  demo_open: zEmpty,
  demo_close: zEmpty,
  demo_reset: zEmpty,
  dashboard_get: z.object({ month: z.string().regex(/^\d{4}-\d{2}$/).nullable().optional() }).strict(),
  ledger_query: zLedgerQuery,
  transaction_detail: z.object({ id: z.string().min(1).max(64) }).strict(),
  transaction_override_set: z
    .object({ id: z.string().min(1).max(64), override: zOverrideInput })
    .strict(),
  transaction_override_clear: z.object({ id: z.string().min(1).max(64) }).strict(),
  merchant_rules_list: zEmpty,
  merchant_rule_create: zMerchantRuleInput,
  merchant_rule_update: z
    .object({ id: z.string().min(1).max(64), rule: zMerchantRuleInput })
    .strict(),
  merchant_rule_delete: z.object({ id: z.string().min(1).max(64) }).strict(),
  manual_payment_get: z.object({ accountId: z.string().min(1).max(64) }).strict(),
  manual_payment_set: z
    .object({ accountId: z.string().min(1).max(64), payment: zManualPaymentInput })
    .strict(),
  manual_payment_clear: z.object({ accountId: z.string().min(1).max(64) }).strict(),
  import_choose_file: z.object({ accountId: z.string().min(1).max(64) }).strict(),
  import_validate: z
    .object({ jobId: z.string().min(1).max(64), mapping: zCsvMapping })
    .strict(),
  import_commit: z
    .object({
      jobId: z.string().min(1).max(64),
      mapping: zCsvMapping,
      acceptReview: z.boolean(),
    })
    .strict(),
  import_cancel: z.object({ jobId: z.string().min(1).max(64) }).strict(),
  import_batches_list: z.object({ limit: z.number().int().min(1).max(50).optional() }).strict(),
  import_rollback: z.object({ batchId: z.string().min(1).max(64) }).strict(),
  export_ledger_csv: z.object({ query: zLedgerQuery }).strict(),
  backup_create: zEmpty,
  backup_list: zEmpty,
  backup_delete: z.object({ id: z.string().min(1).max(200) }).strict(),
  backup_restore: zEmpty,
  portable_export: zEmpty,
  crypto_erase: zEmpty,
  sync_status: zEmpty,
  sync_now: zEmpty,
  sync_runs_list: z.object({ limit: z.number().int().min(1).max(50).optional() }).strict(),
  connection_disconnect: z.object({ connectionId: z.string().min(1).max(64) }).strict(),
  plaid_status: zEmpty,
  plaid_setup: zEmpty,
  plaid_test: zEmpty,
  plaid_link_start: zEmpty,
  plaid_link_status: zEmpty,
  plaid_link_cancel: zEmpty,
  plaid_reconnect: z.object({ connectionId: z.string().min(1).max(64) }).strict(),
  plaid_verification_report: zEmpty,
};

export const responseSchemas: { [K in CommandName]: z.ZodType<CommandDefs[K]["response"]> } = {
  app_info: zAppInfo,
  health_check: zHealthCheck,
  vault_status: zVaultState,
  vault_create: zVaultState,
  vault_unlock: zVaultState,
  vault_lock: zVaultState,
  vault_change_passphrase: zVaultState,
  settings_get: zSettingsView,
  settings_update: zSettingsView,
  security_status: zSecurityView,
  security_events_list: zEventsView,
  accounts_list: zAccountsView,
  account_create: zAccount,
  account_update: zAccount,
  account_archive: zAccount,
  app_quit: z.object({ quitting: z.boolean() }).strict(),
  demo_open: zVaultState,
  demo_close: zVaultState,
  demo_reset: zVaultState,
  dashboard_get: zDashboard,
  ledger_query: zLedgerPage,
  transaction_detail: zTransactionDetail,
  transaction_override_set: zTransactionDetail,
  transaction_override_clear: zTransactionDetail,
  merchant_rules_list: z.object({ rules: z.array(zMerchantRule) }).strict(),
  merchant_rule_create: zMerchantRule,
  merchant_rule_update: zMerchantRule,
  merchant_rule_delete: z.object({ deleted: z.boolean() }).strict(),
  manual_payment_get: z.object({ payment: zManualPayment.nullable() }).strict(),
  manual_payment_set: z.object({ payment: zManualPayment.nullable() }).strict(),
  manual_payment_clear: z.object({ payment: zManualPayment.nullable() }).strict(),
  import_choose_file: zImportFileView,
  import_validate: zValidationPreview,
  import_commit: zImportResult,
  import_cancel: z.object({ cancelled: z.boolean() }).strict(),
  import_batches_list: z.object({ batches: z.array(zImportBatch) }).strict(),
  import_rollback: zImportBatch,
  export_ledger_csv: zExportResult,
  backup_create: zBackupResult,
  backup_list: z.object({ backups: z.array(zBackupItem) }).strict(),
  backup_delete: z.object({ deleted: z.boolean() }).strict(),
  backup_restore: zRestoreResult,
  portable_export: zBackupItem,
  crypto_erase: zVaultState,
  sync_status: zSyncStatus,
  sync_now: zSyncSummary,
  sync_runs_list: z.object({ runs: z.array(zSyncRunView) }).strict(),
  connection_disconnect: z.object({ deleted: z.boolean() }).strict(),
  plaid_status: z.object({ plaid: zPlaidStatus }).strict(),
  plaid_setup: z.object({ plaid: zPlaidStatus }).strict(),
  plaid_test: z.object({ plaid: zPlaidStatus }).strict(),
  plaid_link_start: z.object({ link: zPlaidLinkStatus }).strict(),
  plaid_link_status: z.object({ link: zPlaidLinkStatus }).strict(),
  plaid_link_cancel: z.object({ link: zPlaidLinkStatus }).strict(),
  plaid_reconnect: z.object({ link: zPlaidLinkStatus }).strict(),
  plaid_verification_report: zPlaidVerificationReport,
};

export const zGatewayEnvelope = z
  .object({
    apiVersion: z.literal(API_VERSION),
    data: z.unknown(),
  })
  .strict();

export const zErrorPayload = z
  .object({
    code: z.string().min(1).max(64),
    message: z.string().min(1).max(500),
  })
  .strict();

export function isCommandName(value: string): value is CommandName {
  return Object.prototype.hasOwnProperty.call(responseSchemas, value);
}
