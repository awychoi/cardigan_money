import type { IssuerArt, Theme, ThemeMode } from "./schema";

/** Default card art for issuers a theme does not name. */
export const FALLBACK_ISSUER_ART: IssuerArt = {
  from: "#39424c",
  to: "#1b2026",
  ink: "#f4f7f8",
};

export function issuerSlug(issuer: string): string {
  const slug = issuer
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return slug || "unknown";
}

export function issuerArt(theme: Theme, issuer: string): IssuerArt {
  const key = issuerSlug(issuer);
  return theme.issuers?.[key] ?? theme.issuers?.default ?? FALLBACK_ISSUER_ART;
}

/**
 * The complete CSS custom-property set for one theme + mode.
 *
 * Base colors are set here; tinted washes and borders are derived in CSS with
 * `color-mix(in srgb, …)` so a theme only has to define the base palette.
 */
export function themeVariables(theme: Theme, mode: ThemeMode): Record<string, string> {
  const p = theme.modes[mode];
  const vars: Record<string, string> = {
    "--bg": p.bg,
    "--sidebar": p.sidebar,
    "--surface": p.surface,
    "--surface-2": p.surface2,
    "--surface-3": p.surface3,

    "--text": p.text,
    "--text-2": p.text2,
    "--muted": p.muted,

    "--border": p.border,
    "--border-strong": p.borderStrong,
    "--border-control": p.borderControl,

    "--accent": p.accent,
    "--accent-ink": p.accentInk,
    "--success": p.success,
    "--on-accent": p.onAccent,
    "--warning": p.warning,
    "--danger": p.danger,
    "--focus": p.focus,

    "--shadow": p.shadow,
    "--shadow-strong": p.shadowStrong,

    "--font-sans": theme.type.sans,
    "--font-mono": theme.type.mono,
    "--font-display": theme.type.display,
    "--type-scale": String(theme.type.scale),
    "--display-weight": String(theme.type.displayWeight),
    "--display-tracking": `${theme.type.displayTracking}em`,
    "--line-height": String(theme.type.lineHeight),

    "--radius-control": `${theme.shape.controlRadius}px`,
    "--radius-panel": `${theme.shape.panelRadius}px`,
    "--radius-card": `${theme.shape.cardRadius}px`,
    "--border-width": `${theme.shape.borderWidth}px`,

    "--screen-pad": `${theme.spacing.screen}px`,
    "--card-pad": `${theme.spacing.card}px`,
    "--gap": `${theme.spacing.gap}px`,

    "--motion-fast": `${theme.motion.fast}ms`,
    "--motion-base": `${theme.motion.base}ms`,
    "--ease": theme.motion.easing,

    "--issuer-default-from": (theme.issuers?.default ?? FALLBACK_ISSUER_ART).from,
    "--issuer-default-to": (theme.issuers?.default ?? FALLBACK_ISSUER_ART).to,
    "--issuer-default-ink": (theme.issuers?.default ?? FALLBACK_ISSUER_ART).ink,
  };

  p.chart.forEach((color, index) => {
    vars[`--chart-${index + 1}`] = color;
  });

  for (const [slug, art] of Object.entries(theme.issuers ?? {})) {
    vars[`--issuer-${slug}-from`] = art.from;
    vars[`--issuer-${slug}-to`] = art.to;
    vars[`--issuer-${slug}-ink`] = art.ink;
  }

  return vars;
}

/** Per-card CSS variables for a card face. */
export function issuerVariables(theme: Theme, issuer: string): Record<string, string> {
  const art = issuerArt(theme, issuer);
  return {
    "--issuer-from": art.from,
    "--issuer-to": art.to,
    "--issuer-ink": art.ink,
  };
}
