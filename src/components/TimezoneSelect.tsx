import { useMemo } from "react";
import { timezoneGroups } from "../lib/timezone";

export function TimezoneSelect({
  value,
  onChange,
  disabled,
  id,
  ariaLabel = "Reporting timezone",
  className = "",
}: {
  value: string;
  onChange: (zone: string) => void;
  disabled?: boolean;
  id?: string;
  ariaLabel?: string;
  className?: string;
}) {
  const groups = useMemo(() => timezoneGroups([value]), [value]);

  return (
    <select
      id={id}
      className={`timezone-select ${className}`.trim()}
      aria-label={ariaLabel}
      value={value}
      onChange={(event) => onChange(event.target.value)}
      disabled={disabled}
    >
      {groups.map((group) => (
        <optgroup key={group.label} label={group.label}>
          {group.options.map((option) => (
            <option key={option.zone} value={option.zone}>
              {option.label}
            </option>
          ))}
        </optgroup>
      ))}
    </select>
  );
}
