import { formatMoney } from "../../../lib/formatting";
import type { ChartDatum } from "./chartMath";

export function ChartLegend({
  items,
  currency = "USD",
}: {
  items: ChartDatum[];
  currency?: string;
}) {
  return (
    <ul className="chart-legend" aria-label="Chart legend">
      {items.map((item) => (
        <li className="chart-legend-item" key={item.key}>
          <span className="chart-swatch" style={{ background: item.color }} aria-hidden="true" />
          <span className="chart-legend-label">{item.label}</span>
          <span className="chart-legend-value">{formatMoney(item.valueMinor, currency)}</span>
          <span className="chart-legend-share">{item.share}%</span>
        </li>
      ))}
    </ul>
  );
}
