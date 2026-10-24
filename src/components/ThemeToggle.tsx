export type ThemeChoice = "light" | "dark" | "system";

const LABELS: Record<ThemeChoice, string> = {
  light: "Light",
  dark: "Dark",
  system: "System",
};

export function ThemeToggle({
  theme,
  onChange,
}: {
  theme: ThemeChoice;
  onChange: (next: ThemeChoice) => void;
}) {
  const next: ThemeChoice = theme === "system" ? "light" : theme === "light" ? "dark" : "system";
  return (
    <button
      type="button"
      className="btn btn-ghost theme-toggle"
      onClick={() => onChange(next)}
      aria-label={`Appearance: ${LABELS[theme]}. Switch to ${LABELS[next]}.`}
      title={`Appearance: ${LABELS[theme]}`}
    >
      <span aria-hidden="true">{theme === "dark" ? "◐" : theme === "light" ? "☀" : "◑"}</span>
      <span className="theme-label">{LABELS[theme]}</span>
    </button>
  );
}
