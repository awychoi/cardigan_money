import { formatMoney } from "../../../lib/formatting";
import { ChartTable } from "./ChartTable";
import type { ChartDatum } from "./chartMath";
import "./charts.css";

/** Horizontal ranked composition for themes that prefer bars to a ring. */
export function RankedBars({
  data,
  currency = "USD",
  title = "Where it went",
}: {
  data: ChartDatum[];
  currency?: string;
  title?: string;
}) {
  return (
    <figure className="chart chart-ranked">
      <figcaption className="chart-heading">{title}</figcaption>
      {data.length === 0 ? (
        <p className="chart-empty">No posted spending this month.</p>
      ) : (
        <ol className="ranked-list" aria-label="Spending by category">
          {data.map((item) => (
            <li className="ranked-item" key={item.key}>
              <span className="ranked-label">{item.label}</span>
              <span className="ranked-track" aria-hidden="true">
                <span
                  className="ranked-fill"
                  style={{ background: item.color, width: `${item.share}%` }}
                />
              </span>
              <span className="ranked-value">{formatMoney(item.valueMinor, currency)}</span>
              <span className="ranked-share">{item.share}%</span>
            </li>
          ))}
        </ol>
      )}
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
    </figure>
  );
}
