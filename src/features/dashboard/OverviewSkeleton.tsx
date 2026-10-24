import "./overview.css";

/**
 * Layout-stable skeleton for the Overview while `dashboard_get` is in flight.
 * It mirrors the real grid so the page does not jump when data arrives.
 */
export function OverviewSkeleton() {
  return (
    <section className="overview-skeleton" role="status" aria-label="Loading the encrypted ledger">
      <span className="sr-only">Loading the encrypted ledger…</span>
      <div className="skeleton-panel skeleton-hero">
        <span className="skeleton-block skeleton-line skeleton-line-short" aria-hidden="true" />
        <span className="skeleton-block skeleton-line skeleton-line-hero" aria-hidden="true" />
        <span className="skeleton-block skeleton-line" aria-hidden="true" />
        <span className="skeleton-block skeleton-line skeleton-line-mid" aria-hidden="true" />
      </div>
      <div className="skeleton-panel skeleton-due">
        <span className="skeleton-block skeleton-line skeleton-line-short" aria-hidden="true" />
        <span className="skeleton-block skeleton-strip" aria-hidden="true" />
        <span className="skeleton-block skeleton-line" aria-hidden="true" />
        <span className="skeleton-block skeleton-line" aria-hidden="true" />
        <span className="skeleton-block skeleton-line skeleton-line-mid" aria-hidden="true" />
      </div>
      <div className="skeleton-panel skeleton-composition">
        <span className="skeleton-block skeleton-line skeleton-line-short" aria-hidden="true" />
        <span className="skeleton-block skeleton-ring" aria-hidden="true" />
      </div>
      <div className="skeleton-panel skeleton-trend">
        <span className="skeleton-block skeleton-line skeleton-line-short" aria-hidden="true" />
        <span className="skeleton-block skeleton-chart" aria-hidden="true" />
      </div>
      <div className="skeleton-panel skeleton-balances">
        <span className="skeleton-block skeleton-line skeleton-line-short" aria-hidden="true" />
        <span className="skeleton-block skeleton-line skeleton-line-hero" aria-hidden="true" />
        <span className="skeleton-block skeleton-line" aria-hidden="true" />
        <span className="skeleton-block skeleton-line" aria-hidden="true" />
      </div>
      <div className="skeleton-panel skeleton-attention">
        <span className="skeleton-block skeleton-line skeleton-line-short" aria-hidden="true" />
        <span className="skeleton-block skeleton-line" aria-hidden="true" />
        <span className="skeleton-block skeleton-line skeleton-line-mid" aria-hidden="true" />
      </div>
    </section>
  );
}
