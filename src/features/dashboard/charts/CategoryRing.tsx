import { formatMoney } from "../../../lib/formatting";
import { ChartLegend } from "./ChartLegend";
import { ChartTable } from "./ChartTable";
import { ringSlices, type ChartDatum } from "./chartMath";
import "./charts.css";

/**
 * Gross spending by category as a proportional donut. The center shows gross
 * because a ring cannot represent negative refunds; the footnote carries the
 * refunds-to-net arithmetic and the hero keeps the net figure.
 */
export function CategoryRing({
  data,
  totalMinor,
  refundsMinor,
  netMinor,
  currency = "USD",
  title = "Where it went",
}: {
  data: ChartDatum[];
  totalMinor: number;
  refundsMinor: number;
  netMinor: number;
  currency?: string;
  title?: string;
}) {
  const slices = ringSlices(data.map((item) => item.share));
  const empty = data.length === 0 || totalMinor <= 0;
  const ariaLabel = empty
    ? "No posted spending in this month."
    : `Gross spending by category: ${data
        .map(
          (item) =>
            `${item.label} ${formatMoney(item.valueMinor, currency)}, ${item.share} percent`,
        )
        .join("; ")}. Total ${formatMoney(totalMinor, currency)}.`;

  return (
    <figure className="chart chart-ring">
      <figcaption className="chart-heading">{title}</figcaption>
      <div className="ring-visual">
        <svg className="ring-svg" viewBox="0 0 42 42" role="img" aria-label={ariaLabel}>
          <circle className="ring-track" cx="21" cy="21" r="15.9155" />
          {empty
            ? null
            : slices.map((slice, index) =>
                slice.share > 0 ? (
                  <circle
                    key={data[index].key}
                    className="ring-slice"
                    cx="21"
                    cy="21"
                    r="15.9155"
                    pathLength={100}
                    style={{
                      stroke: data[index].color,
                      strokeDasharray: `${slice.share} ${100 - slice.share}`,
                      strokeDashoffset: -slice.offset,
                    }}
                  />
                ) : null,
              )}
        </svg>
        <div className="ring-center" aria-hidden="true">
          <span className="ring-total">{empty ? "—" : formatMoney(totalMinor, currency)}</span>
          <span className="ring-caption">{empty ? "no spending" : "gross"}</span>
        </div>
      </div>

      {empty ? (
        <p className="chart-empty">No posted spending this month.</p>
      ) : (
        <>
          <ChartLegend items={data} currency={currency} />
          <p className="chart-footnote">
            {refundsMinor < 0
              ? `${formatMoney(Math.abs(refundsMinor), currency)} refunds → ${formatMoney(
                  netMinor,
                  currency,
                )} net`
              : `${formatMoney(netMinor, currency)} net`}
          </p>
          <ChartTable
            caption={`${title}: category spending, gross, refunds, and net`}
            columns={["Category", "Gross", "Refunds", "Net", "Rows", "Share"]}
            rows={data.map((item) => [
              item.label,
              formatMoney(item.valueMinor, currency),
              formatMoney(Math.abs(item.refundsMinor), currency),
              formatMoney(item.netMinor, currency),
              item.count,
              `${item.share}%`,
            ])}
          />
        </>
      )}
    </figure>
  );
}
