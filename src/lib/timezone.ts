/**
 * Reporting-timezone helpers for onboarding and settings.
 *
 * The app stores IANA zone ids (for example `America/New_York`) because Rust
 * validates them with `chrono-tz`. This module turns those ids into option
 * labels non-technical users can scan: a friendly zone name, a nearby city, and
 * the raw id in parentheses for verification.
 */

export const FALLBACK_TIMEZONE = "America/New_York";

export interface TimezoneDetection {
  zone: string;
  detected: boolean;
  resolved: string | null;
}

export interface TimezoneOption {
  zone: string;
  label: string;
  city: string;
  group: string;
}

export interface TimezoneGroup {
  label: string;
  options: TimezoneOption[];
}

const GROUP_ORDER = [
  "UTC",
  "Americas",
  "Europe",
  "Africa",
  "Asia",
  "Australia & Pacific",
  "Atlantic",
  "Indian Ocean",
  "Other",
];

const AREA_GROUPS: Record<string, string> = {
  America: "Americas",
  Europe: "Europe",
  Africa: "Africa",
  Asia: "Asia",
  Australia: "Australia & Pacific",
  Pacific: "Australia & Pacific",
  Atlantic: "Atlantic",
  Indian: "Indian Ocean",
  Arctic: "Other",
  Antarctica: "Other",
  Etc: "Other",
};

/**
 * A small, readable fallback for WebViews without `Intl.supportedValuesOf`.
 * When the API exists we still include these ids and the full IANA list.
 */
const FALLBACK_ZONES = [
  "America/New_York",
  "America/Chicago",
  "America/Denver",
  "America/Phoenix",
  "America/Los_Angeles",
  "America/Anchorage",
  "Pacific/Honolulu",
  "America/Toronto",
  "America/Vancouver",
  "America/Edmonton",
  "America/Winnipeg",
  "America/Halifax",
  "America/Mexico_City",
  "America/Sao_Paulo",
  "America/Argentina/Buenos_Aires",
  "America/Bogota",
  "America/Lima",
  "America/Santiago",
  "Europe/London",
  "Europe/Dublin",
  "Europe/Lisbon",
  "Europe/Paris",
  "Europe/Berlin",
  "Europe/Madrid",
  "Europe/Rome",
  "Europe/Amsterdam",
  "Europe/Zurich",
  "Europe/Stockholm",
  "Europe/Oslo",
  "Europe/Copenhagen",
  "Europe/Helsinki",
  "Europe/Warsaw",
  "Europe/Athens",
  "Europe/Istanbul",
  "Europe/Kyiv",
  "Europe/Moscow",
  "Africa/Cairo",
  "Africa/Johannesburg",
  "Africa/Lagos",
  "Africa/Nairobi",
  "Africa/Casablanca",
  "Asia/Jerusalem",
  "Asia/Dubai",
  "Asia/Riyadh",
  "Asia/Karachi",
  "Asia/Kolkata",
  "Asia/Dhaka",
  "Asia/Bangkok",
  "Asia/Jakarta",
  "Asia/Singapore",
  "Asia/Hong_Kong",
  "Asia/Shanghai",
  "Asia/Taipei",
  "Asia/Seoul",
  "Asia/Tokyo",
  "Asia/Manila",
  "Australia/Sydney",
  "Australia/Melbourne",
  "Australia/Brisbane",
  "Australia/Adelaide",
  "Australia/Perth",
  "Pacific/Auckland",
  "Pacific/Fiji",
  "UTC",
];

let zoneListCache: string[] | null = null;
const labelCache = new Map<string, string>();

export function isValidTimeZone(zone: string): boolean {
  if (!zone) return false;
  try {
    new Intl.DateTimeFormat("en-US", { timeZone: zone }).format(0);
    return true;
  } catch {
    return false;
  }
}

/** Detect the Mac's current IANA zone, falling back to New York. */
export function detectTimeZone(): TimezoneDetection {
  let resolved: string | null = null;
  try {
    resolved = Intl.DateTimeFormat().resolvedOptions().timeZone || null;
  } catch {
    resolved = null;
  }
  if (resolved && isValidTimeZone(resolved)) {
    return { zone: resolved, detected: true, resolved };
  }
  return { zone: FALLBACK_TIMEZONE, detected: false, resolved };
}

function supportedTimeZones(): string[] {
  const candidate = (
    Intl as unknown as {
      supportedValuesOf?: (key: string) => string[];
    }
  ).supportedValuesOf;
  if (typeof candidate !== "function") return [];
  try {
    const values = candidate.call(Intl, "timeZone");
    return Array.isArray(values) ? values.filter((value) => typeof value === "string") : [];
  } catch {
    return [];
  }
}

function allTimeZones(): string[] {
  if (zoneListCache) return zoneListCache;
  const zones = new Set<string>(["UTC", ...FALLBACK_ZONES, ...supportedTimeZones()]);
  zoneListCache = [...zones].filter(isValidTimeZone);
  return zoneListCache;
}

function cityFor(zone: string): string {
  if (zone === "UTC") return "UTC";
  const parts = zone.split("/");
  return (parts.at(-1) ?? zone).replaceAll("_", " ");
}

function groupFor(zone: string): string {
  if (zone === "UTC") return "UTC";
  return AREA_GROUPS[zone.split("/")[0] ?? ""] ?? "Other";
}

function friendlyNameFor(zone: string): string {
  if (zone === "UTC") return "Coordinated Universal Time";
  try {
    const parts = new Intl.DateTimeFormat("en-US", {
      timeZone: zone,
      timeZoneName: "longGeneric",
    }).formatToParts(new Date());
    const name = parts.find((part) => part.type === "timeZoneName")?.value;
    if (name && name !== zone) return name;
  } catch {
    // Older WebViews may not support longGeneric; the city still explains it.
  }
  return cityFor(zone);
}

/** "Eastern Time — New York (America/New_York)" */
export function timeZoneOptionLabel(zone: string): string {
  if (zone === "UTC") return "Coordinated Universal Time (UTC)";
  const cached = labelCache.get(zone);
  if (cached) return cached;
  const label = `${friendlyNameFor(zone)} — ${cityFor(zone)} (${zone})`;
  labelCache.set(zone, label);
  return label;
}

/**
 * Grouped, de-duplicated options for a select. `include` lets the current
 * selection appear even if the WebView does not enumerate it.
 */
export function timezoneGroups(include: string[] = []): TimezoneGroup[] {
  const zones = new Set([...allTimeZones(), ...include.filter(isValidTimeZone)]);
  const groups = new Map<string, TimezoneOption[]>();
  for (const zone of zones) {
    const group = groupFor(zone);
    const options = groups.get(group) ?? [];
    options.push({
      zone,
      label: timeZoneOptionLabel(zone),
      city: cityFor(zone),
      group,
    });
    groups.set(group, options);
  }
  return GROUP_ORDER.filter((label) => groups.has(label)).map((label) => ({
    label,
    options: (groups.get(label) ?? []).sort((a, b) => a.label.localeCompare(b.label)),
  }));
}

/** Current UTC offset as a readable `UTC-04:00` / `UTC+05:30` string. */
export function timeZoneOffsetLabel(zone: string, now: Date = new Date()): string {
  try {
    const parts = new Intl.DateTimeFormat("en-US", {
      timeZone: zone,
      timeZoneName: "shortOffset",
    }).formatToParts(now);
    const raw = parts.find((part) => part.type === "timeZoneName")?.value;
    if (!raw) return "";
    if (raw === "GMT" || raw === "UTC") return "UTC+00:00";
    const match = /^GMT([+-])(\d{1,2})(?::(\d{2}))?$/.exec(raw);
    if (!match) return raw;
    const [, sign, hour, minute = "00"] = match;
    return `UTC${sign}${hour.padStart(2, "0")}:${minute}`;
  } catch {
    return "";
  }
}

/** "Sunday, October 25 at 10:08 PM (UTC-04:00)" in the selected zone. */
export function formatTimeZonePreview(zone: string, now: Date = new Date()): string {
  try {
    const time = new Intl.DateTimeFormat("en-US", {
      weekday: "long",
      month: "long",
      day: "numeric",
      hour: "numeric",
      minute: "2-digit",
      timeZone: zone,
    }).format(now);
    const offset = timeZoneOffsetLabel(zone, now);
    return offset ? `${time} (${offset})` : time;
  } catch {
    return "";
  }
}

/** Short human label for confirmation screens and settings hints. */
export function timeZoneSummary(zone: string): string {
  return timeZoneOptionLabel(zone);
}
