import { afterEach, describe, expect, it, vi } from "vitest";
import {
  detectTimeZone,
  FALLBACK_TIMEZONE,
  formatTimeZonePreview,
  isValidTimeZone,
  timeZoneOffsetLabel,
  timeZoneOptionLabel,
  timezoneGroups,
} from "./timezone";

describe("timezone helpers", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("labels a zone with a friendly name, city, and IANA id", () => {
    const label = timeZoneOptionLabel("America/New_York");
    expect(label).toContain("Eastern Time");
    expect(label).toContain("New York");
    expect(label).toContain("(America/New_York)");
  });

  it("detects a valid zone on this machine", () => {
    const detection = detectTimeZone();
    expect(isValidTimeZone(detection.zone)).toBe(true);
    expect(detection.detected).toBe(true);
  });

  it("falls back to New York when detection returns an invalid zone", () => {
    vi.spyOn(Intl.DateTimeFormat.prototype, "resolvedOptions").mockReturnValue({
      timeZone: "Mars/Olympus",
    } as ReturnType<Intl.DateTimeFormat["resolvedOptions"]>);
    expect(detectTimeZone()).toMatchObject({
      zone: FALLBACK_TIMEZONE,
      detected: false,
      resolved: "Mars/Olympus",
    });
  });

  it("formats the current offset and a readable local-time preview", () => {
    const winter = new Date("2026-01-15T12:00:00Z");
    expect(timeZoneOffsetLabel("America/New_York", winter)).toBe("UTC-05:00");
    const preview = formatTimeZonePreview("America/New_York", winter);
    expect(preview).toContain("January");
    expect(preview).toContain("UTC-05:00");
  });

  it("groups options and keeps the selected zone present exactly once", () => {
    const groups = timezoneGroups(["America/New_York"]);
    expect(groups.map((group) => group.label)).toContain("Americas");
    const zones = groups.flatMap((group) => group.options.map((option) => option.zone));
    expect(zones).toContain("America/New_York");
    expect(new Set(zones).size).toBe(zones.length);
  });
});
