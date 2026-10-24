import { useEffect, useMemo, useState } from "react";
import {
  createColumnHelper,
  flexRender,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { command, type Dashboard, type LedgerPage, type LedgerRow } from "../../lib/bridge";
import { friendlyError } from "../../lib/bridge/errors";
import { formatDateOnly, formatMoney, formatMonthRange, monthLabel } from "../../lib/formatting";
import { Button } from "../../components/Button";
import { TransactionEditor } from "./TransactionEditor";

type Mode = "transactions" | "suppliers";
type Status = "all" | "pending" | "posted";

const columnHelper = createColumnHelper<LedgerRow>();

export function LedgerView({
  dashboard,
  presetAccountId,
  presetSearch,
  presetNonce,
  onClearPreset,
  onChanged,
  readOnly,
}: {
  dashboard: Dashboard;
  presetAccountId: string | null;
  presetSearch: string | null;
  presetNonce: number;
  onClearPreset: () => void;
  onChanged: () => void;
  readOnly: boolean;
}) {
  const [mode, setMode] = useState<Mode>("transactions");
  const [searchInput, setSearchInput] = useState("");
  const [search, setSearch] = useState("");
  const [account, setAccount] = useState<string>("");
  const [category, setCategory] = useState<string>("");
  const [status, setStatus] = useState<Status>("all");
  const [txType, setTxType] = useState<string>("");
  const [sort, setSort] = useState<string>("date_desc");
  const [pageSize, setPageSize] = useState(50);
  const [page, setPage] = useState(0);
  const [result, setResult] = useState<LedgerPage | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<LedgerRow | null>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const [exportConfirm, setExportConfirm] = useState(false);
  const [exportMessage, setExportMessage] = useState<string | null>(null);

  useEffect(() => {
    const timeout = window.setTimeout(() => {
      setSearch(searchInput);
      setPage(0);
    }, 250);
    return () => window.clearTimeout(timeout);
  }, [searchInput]);

  useEffect(() => {
    if (presetNonce === 0) return;
    setMode("transactions");
    setAccount(presetAccountId ?? "");
    setSearchInput(presetSearch ?? "");
    setSearch(presetSearch ?? "");
    setPage(0);
    setSelected(null);
  }, [presetNonce, presetAccountId, presetSearch]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    (async () => {
      try {
        const response = await command("ledger_query", {
          startDate: dashboard.startDate,
          endDateExclusive: dashboard.endDateExclusive,
          accountIds: account ? [account] : undefined,
          category: category || null,
          status,
          txType: txType
            ? (txType as
                | "purchase"
                | "refund"
                | "fee"
                | "interest"
                | "payment"
                | "transfer"
                | "other")
            : undefined,
          search: search || null,
          sort: sort as "date_desc" | "date_asc" | "amount_desc" | "amount_asc" | "merchant_asc",
          page,
          pageSize,
        });
        if (!cancelled) setResult(response);
      } catch (caught) {
        if (!cancelled) {
          setError(friendlyError(caught));
          setResult(null);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [
    dashboard.startDate,
    dashboard.endDateExclusive,
    account,
    category,
    status,
    txType,
    search,
    sort,
    page,
    pageSize,
    reloadToken,
  ]);

  const columns = useMemo(
    () => [
      columnHelper.accessor("reportingDate", {
        header: "Date",
        cell: (info) => formatDateOnly(info.getValue()),
      }),
      columnHelper.accessor("merchant", { header: "Merchant" }),
      columnHelper.accessor("accountName", {
        header: "Card",
        cell: (info) => (
          <span>
            {info.getValue()}
            {info.row.original.accountLastFour ? ` · ${info.row.original.accountLastFour}` : ""}
          </span>
        ),
      }),
      columnHelper.accessor("category", {
        header: "Category",
        cell: (info) => info.getValue() ?? "Uncategorized",
      }),
      columnHelper.accessor("pending", {
        header: "Status",
        cell: (info) => {
          if (info.row.original.excluded)
            return <span className="status-pill status-warn">Excluded</span>;
          return info.getValue() ? (
            <span className="status-pill status-warn">Pending</span>
          ) : (
            <span className="status-pill status-ok">Posted</span>
          );
        },
      }),
      columnHelper.accessor("amountMinor", {
        header: "Amount",
        cell: (info) => (
          <span className={info.getValue() < 0 ? "amount-negative" : "amount"}>
            {formatMoney(info.getValue(), info.row.original.currency)}
          </span>
        ),
      }),
    ],
    [],
  );

  const table = useReactTable({
    data: result?.rows ?? [],
    columns,
    getCoreRowModel: getCoreRowModel(),
  });

  function clearFilters() {
    setSearchInput("");
    setSearch("");
    setAccount("");
    setCategory("");
    setStatus("all");
    setTxType("");
    setSort("date_desc");
    setPage(0);
    setSelected(null);
    onClearPreset();
  }

  const totalPages = result ? Math.max(1, Math.ceil(result.rowCount / result.pageSize)) : 1;

  function refreshAfterEdit() {
    setReloadToken((value) => value + 1);
    onChanged();
  }

  async function exportCsv() {
    setError(null);
    setExportMessage(null);
    try {
      const response = await command("export_ledger_csv", {
        query: {
          startDate: dashboard.startDate,
          endDateExclusive: dashboard.endDateExclusive,
          accountIds: account ? [account] : undefined,
          category: category || null,
          status,
          txType: txType
            ? (txType as
                | "purchase"
                | "refund"
                | "fee"
                | "interest"
                | "payment"
                | "transfer"
                | "other")
            : undefined,
          search: search || null,
          sort: sort as "date_desc" | "date_asc" | "amount_desc" | "amount_asc" | "merchant_asc",
          page: 0,
          pageSize,
        },
      });
      setExportConfirm(false);
      setExportMessage(
        response.rows === 1
          ? "Exported 1 row as plaintext CSV."
          : `Exported ${response.rows} rows as plaintext CSV.`,
      );
    } catch (caught) {
      setError(friendlyError(caught));
    }
  }

  return (
    <section className="dashboard" aria-labelledby="transactions-title">
      <header className="page-heading">
        <div>
          <h1 id="transactions-title">
            {mode === "transactions" ? "Transactions" : "By supplier"}
          </h1>
          <p className="muted">
            {monthLabel(dashboard.month)} ·{" "}
            {formatMonthRange(dashboard.startDate, dashboard.endDateExclusive)}
          </p>
        </div>
        <div className="heading-actions">
          {!readOnly ? (
            <Button variant="ghost" className="btn-small" onClick={() => setExportConfirm(true)}>
              Export CSV…
            </Button>
          ) : null}
          <div className="segmented" role="group" aria-label="Ledger view">
            <button
              type="button"
              aria-pressed={mode === "transactions"}
              className={mode === "transactions" ? "segment segment-active" : "segment"}
              onClick={() => setMode("transactions")}
            >
              Transactions
            </button>
            <button
              type="button"
              aria-pressed={mode === "suppliers"}
              className={mode === "suppliers" ? "segment segment-active" : "segment"}
              onClick={() => setMode("suppliers")}
            >
              By supplier
            </button>
          </div>
        </div>
      </header>

      {exportMessage ? (
        <p className="inline-notice" role="status">
          {exportMessage}
        </p>
      ) : null}

      {exportConfirm ? (
        <div className="export-warning" role="alert">
          <strong>Plaintext export.</strong>
          <p>
            This writes a plaintext CSV to a location you choose. SQLCipher does not protect it, and
            cloud-synced folders are outside this app's control. Formula-injection protection is
            applied to text fields. The file is written owner-only (0600) and is not auto-opened.
          </p>
          <div className="button-row">
            <Button variant="primary" onClick={() => void exportCsv()}>
              Choose destination…
            </Button>
            <Button variant="ghost" onClick={() => setExportConfirm(false)}>
              Cancel
            </Button>
          </div>
        </div>
      ) : null}

      {mode === "transactions" ? (
        <>
          <div className="filter-bar" role="search">
            <label className="filter-field filter-search">
              <span className="field-label">Search</span>
              <input
                aria-label="Search transactions"
                value={searchInput}
                onChange={(event) => setSearchInput(event.target.value)}
                placeholder="Merchant or description"
                maxLength={120}
              />
            </label>
            <label className="filter-field">
              <span className="field-label">Card</span>
              <select
                aria-label="Filter by card"
                value={account}
                onChange={(event) => {
                  setAccount(event.target.value);
                  setPage(0);
                }}
              >
                <option value="">All cards</option>
                {dashboard.overview.accounts.map((item) => (
                  <option key={item.accountId} value={item.accountId}>
                    {item.displayName}
                  </option>
                ))}
              </select>
            </label>
            <label className="filter-field">
              <span className="field-label">Category</span>
              <select
                aria-label="Filter by category"
                value={category}
                onChange={(event) => {
                  setCategory(event.target.value);
                  setPage(0);
                }}
              >
                <option value="">All categories</option>
                {dashboard.summary.categories.map((item) => (
                  <option key={item.key} value={item.key}>
                    {item.key}
                  </option>
                ))}
              </select>
            </label>
            <label className="filter-field">
              <span className="field-label">Status</span>
              <select
                aria-label="Filter by status"
                value={status}
                onChange={(event) => {
                  setStatus(event.target.value as Status);
                  setPage(0);
                }}
              >
                <option value="all">All</option>
                <option value="posted">Posted</option>
                <option value="pending">Pending</option>
              </select>
            </label>
            <label className="filter-field">
              <span className="field-label">Type</span>
              <select
                aria-label="Filter by type"
                value={txType}
                onChange={(event) => {
                  setTxType(event.target.value);
                  setPage(0);
                }}
              >
                <option value="">All types</option>
                <option value="purchase">Purchases</option>
                <option value="refund">Refunds</option>
                <option value="fee">Fees</option>
                <option value="interest">Interest</option>
                <option value="payment">Payments</option>
                <option value="transfer">Transfers</option>
                <option value="other">Other</option>
              </select>
            </label>
            <label className="filter-field">
              <span className="field-label">Sort</span>
              <select
                aria-label="Sort transactions"
                value={sort}
                onChange={(event) => {
                  setSort(event.target.value);
                  setPage(0);
                }}
              >
                <option value="date_desc">Newest first</option>
                <option value="date_asc">Oldest first</option>
                <option value="amount_desc">Largest amount</option>
                <option value="amount_asc">Smallest amount</option>
                <option value="merchant_asc">Merchant A–Z</option>
              </select>
            </label>
            <Button variant="ghost" className="btn-small" onClick={clearFilters}>
              Clear filters
            </Button>
          </div>

          <div className="ledger-meta" role="status">
            <span>{result?.rowCount ?? 0} rows</span>
            <span>
              Eligible subtotal:{" "}
              <strong className="amount">{formatMoney(result?.eligibleSubtotalMinor ?? 0)}</strong>
            </span>
            <span>
              Pending subtotal:{" "}
              <strong className="amount">{formatMoney(result?.pendingSubtotalMinor ?? 0)}</strong>
            </span>
            {account || category || status !== "all" || txType || search ? (
              <span className="status-pill status-info">Filtered</span>
            ) : null}
          </div>

          {error ? (
            <p className="inline-error" role="alert">
              {error}
            </p>
          ) : null}

          <div className={selected ? "ledger-layout" : "ledger-layout ledger-layout-full"}>
            <div className="table-wrap">
              <table className="ledger-table">
                <caption className="sr-only">
                  Transactions for {monthLabel(dashboard.month)}
                </caption>
                <thead>
                  {table.getHeaderGroups().map((headerGroup) => (
                    <tr key={headerGroup.id}>
                      {headerGroup.headers.map((header) => (
                        <th key={header.id} scope="col">
                          {header.isPlaceholder
                            ? null
                            : flexRender(header.column.columnDef.header, header.getContext())}
                        </th>
                      ))}
                    </tr>
                  ))}
                </thead>
                <tbody>
                  {table.getRowModel().rows.map((row) => (
                    <tr
                      key={row.id}
                      className={selected?.id === row.original.id ? "row-selected" : undefined}
                      tabIndex={0}
                      onClick={() => setSelected(row.original)}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" || event.key === " ") {
                          event.preventDefault();
                          setSelected(row.original);
                        }
                      }}
                    >
                      {row.getVisibleCells().map((cell) => (
                        <td key={cell.id}>
                          {flexRender(cell.column.columnDef.cell, cell.getContext())}
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
              {loading ? <p className="muted ledger-loading">Loading transactions…</p> : null}
              {!loading && result && result.rows.length === 0 ? (
                <p className="empty-row">
                  No transactions match these filters. Try clearing them or another month.
                </p>
              ) : null}
            </div>

            {selected ? (
              <TransactionEditor
                row={selected}
                readOnly={readOnly}
                onSaved={refreshAfterEdit}
                onClose={() => setSelected(null)}
              />
            ) : null}
          </div>

          <div className="pagination">
            <Button
              variant="ghost"
              className="btn-small"
              disabled={page === 0}
              onClick={() => setPage((value) => Math.max(0, value - 1))}
            >
              Previous
            </Button>
            <span>
              Page {page + 1} of {totalPages}
            </span>
            <Button
              variant="ghost"
              className="btn-small"
              disabled={page + 1 >= totalPages}
              onClick={() => setPage((value) => value + 1)}
            >
              Next
            </Button>
            <label className="filter-field">
              <span className="field-label">Rows</span>
              <select
                aria-label="Rows per page"
                value={pageSize}
                onChange={(event) => {
                  setPageSize(Number(event.target.value));
                  setPage(0);
                }}
              >
                <option value={25}>25</option>
                <option value={50}>50</option>
                <option value={100}>100</option>
              </select>
            </label>
          </div>
        </>
      ) : (
        <div className="table-wrap">
          <p className="muted">
            Gross purchases, refunds, and net spend per supplier for {monthLabel(dashboard.month)}.
            Click a supplier to inspect its ledger rows.
          </p>
          {dashboard.summary.suppliers.length === 0 ? (
            <p className="empty-row">No eligible spending in this month.</p>
          ) : (
            <table className="ledger-table suppliers-table">
              <caption className="sr-only">Supplier totals</caption>
              <thead>
                <tr>
                  <th scope="col">Supplier</th>
                  <th scope="col">Gross</th>
                  <th scope="col">Refunds</th>
                  <th scope="col">Net</th>
                  <th scope="col">Transactions</th>
                </tr>
              </thead>
              <tbody>
                {dashboard.summary.suppliers.map((supplier) => (
                  <tr
                    key={supplier.key}
                    tabIndex={0}
                    onClick={() => {
                      setMode("transactions");
                      setSearchInput(supplier.key);
                      setSearch(supplier.key);
                      setPage(0);
                    }}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" || event.key === " ") {
                        event.preventDefault();
                        setMode("transactions");
                        setSearchInput(supplier.key);
                        setSearch(supplier.key);
                        setPage(0);
                      }
                    }}
                  >
                    <td>{supplier.key}</td>
                    <td className="amount">{formatMoney(supplier.grossMinor)}</td>
                    <td className="amount-negative">{formatMoney(supplier.refundsMinor)}</td>
                    <td className="amount">{formatMoney(supplier.netMinor)}</td>
                    <td>{supplier.count}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          <p className="fine-print">
            Supplier totals reconcile exactly to the eligible ledger rows:{" "}
            {formatMoney(dashboard.summary.reconciliation.supplierNetMinor)}.
          </p>
        </div>
      )}
    </section>
  );
}
