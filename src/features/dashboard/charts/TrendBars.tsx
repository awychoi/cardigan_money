import type { CSSProperties } from "react";
import type { MonthTotal } from "../../../lib/bridge";
import { formatMoney, monthLabel, monthShortLabel } from "../../../lib/formatting";
import { ChartTable } from "./ChartTable";
import { barScale } from "./chartMath";
import "./charts.css";

function statusFor(month: MonthTotal, currentMonth: string): string {
  if (month.month === currentMonth) return "So far";
  return month.month > currentMonth ? "Upcoming" : "Complete";
}

/**
 * Monthly net spending as columns on a shared zero baseline. Negative months
 * render below the baseline; the current month is hatched and labelled
 * "so far"; the selected month is outlined.
 */
export function TrendBars({
  months,
  selectedMonth,
  currentMonth,
  currency = "USD",
  title = "Monthly net spending",
}: {
  months: MonthTotal[];
  selectedMonth: string;
  currentMonth: string;
  currency?: string;
  title?: string;
}) {
  const scale = barScale(months.map((month) => month.netMinor));
  const ariaLabel =
    months.length === 0
      ? "No monthly trend data."
      : `${title}, ${monthLabel(months[0].month)} to ${monthLabel(
          months[months.length - 1].month,
        )}: ${months
          .map(
            (month) => `${monthShortLabel(month.month)} ${formatMoney(month.netMinor, currency)}`,
          )
          .join(", ")}.`;

  return (
    <figure className="chart chart-trend">
      <figcaption className="chart-heading">{title}</figcaption>
      {months.length === 0 ? (
        <p className="chart-empty">No trend data yet.</p>
      ) : (
        <div className="trend-plot">
          <div
            className="trend-bars"
            role="img"
            aria-label={ariaLabel}
            style={{ "--trend-zero": `${scale.zeroPercent}%` } as CSSProperties}
          >
            <span className="trend-zero" aria-hidden="true" />
            {months.map((month, index) => {
              const bar = scale.bars[index];
              const current = month.month === currentMonth;
              const future = month.month > currentMonth;
              const selected = month.month === selectedMonth;
              const className = [
                "trend-col",
                current ? "trend-col-current" : "",
                future ? "trend-col-future" : "",
                selected ? "trend-col-selected" : "",
              ]
                .filter(Boolean)
                .join(" ");
              return (
                <div
                  className={className}
                  key={month.month}
                  title={`${monthLabel(month.month)}: ${formatMoney(month.netMinor, currency)} net`}
                >
                  {bar.heightPercent === 0 ? (
                    <span
                      className="trend-bar trend-bar-zero"
                      style={{ top: `calc(${scale.zeroPercent}% - 1px)` }}
                    />
                  ) : (
                    <span
                      className={`trend-bar${current ? " trend-bar-current" : ""}`}
                      style={{
                        height: `${bar.heightPercent}%`,
                        ...(bar.negative
                          ? { top: `${scale.zeroPercent}%` }
                          : { bottom: `${100 - scale.zeroPercent}%` }),
                      }}
                    />
                  )}
                </div>
              );
            })}
          </div>
          <div className="trend-labels" aria-hidden="true">
            {months.map((month) => {
              const current = month.month === currentMonth;
              const selected = month.month === selectedMonth;
              return (
                <span
                  className={`trend-label${selected ? " trend-label-selected" : ""}`}
                  key={month.month}
                >
                  {monthShortLabel(month.month)}
                  {current ? <span className="trend-so-far">so far</span> : null}
                </span>
              );
            })}
          </div>
        </div>
      )}
      <ChartTable
        caption={`${title}: monthly gross, refunds, net, and rows`}
        columns={["Month", "Gross", "Refunds", "Net", "Rows", "Status"]}
        rows={months.map((month) => [
          monthLabel(month.month),
          formatMoney(month.grossMinor, currency),
          formatMoney(Math.abs(month.refundsMinor), currency),
          formatMoney(month.netMinor, currency),
          month.count,
          statusFor(month, currentMonth),
        ])}
      />
    </figure>
  );
}
