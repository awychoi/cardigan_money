import { BrandMark } from "./BrandMark";
import { THEMES } from "../theme";
import type { ThemeMode } from "../theme/schema";

/**
 * Theme picker. Everything it shows comes from the theme documents, so a new
 * file in src/themes appears here with no code change.
 */
export function ThemePicker({
  value,
  mode,
  navigation,
  onChange,
}: {
  value: string;
  mode: ThemeMode;
  navigation: "sidebar" | "topbar";
  onChange: (themeId: string) => void;
}) {
  return (
    <>
      <div className="theme-grid" role="group" aria-label="Theme">
        {THEMES.map((theme) => {
          const palette = theme.modes[mode];
          const selected = theme.id === value;
          return (
            <button
              key={theme.id}
              type="button"
              className={`theme-card${selected ? " theme-card-selected" : ""}`}
              aria-pressed={selected}
              onClick={() => onChange(theme.id)}
            >
              <span className="theme-preview" aria-hidden="true" style={{ background: palette.bg }}>
                {navigation === "sidebar" ? (
                  <span className="theme-preview-side" style={{ background: palette.sidebar }} />
                ) : null}
                <span className="theme-preview-body">
                  <span
                    className="theme-preview-bar"
                    style={{ background: palette.text, width: "46%" }}
                  />
                  <span
                    className="theme-preview-bar"
                    style={{ background: palette.muted, width: "72%" }}
                  />
                  <span
                    className="theme-preview-bar"
                    style={{ background: palette.border, width: "84%" }}
                  />
                  <span className="theme-preview-chip" style={{ background: palette.accent }} />
                </span>
              </span>
              <span className="theme-card-text">
                <span className="theme-card-heading">
                  <BrandMark className="theme-card-mark" themeId={theme.id} />
                  <span className="theme-card-name">{theme.name}</span>
                </span>
                <span className="theme-card-desc">{theme.description ?? ""}</span>
              </span>
            </button>
          );
        })}
      </div>
      <p className="fine-print">
        Themes are data files in <code>src/themes</code>. Add one plus a placket colorway, run{" "}
        <code>pnpm theme:check</code>, and it appears here. Authoring guide:{" "}
        <code>docs/design/theming.md</code>.
      </p>
    </>
  );
}
