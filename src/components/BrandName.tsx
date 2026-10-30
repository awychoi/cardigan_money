/**
 * The cardigan_money wordmark.
 *
 * Rendered as text so it stays selectable and accessible. The type and the
 * separator colour come from the active theme: mono for the mark, the accent
 * for the underscore, so every theme keeps its own palette.
 */
export function BrandName({ className = "brand-name" }: { className?: string }) {
  return (
    <span className={className} aria-label="cardigan_money">
      {"cardigan"}
      <span className="brand-name-separator">_</span>
      {"money"}
    </span>
  );
}
