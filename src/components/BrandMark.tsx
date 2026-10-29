import { brandMarkFor, brandThemeFor } from "../brand/marks";

/**
 * The cardigan_money mark: the chosen placket in the current theme's colorway,
 * cropped to its macOS tile so it fills the slot at every size (sidebar, vault,
 * boot screen). A missing or unrecognized colorway falls back to Blueprint.
 */
export function BrandMark({
  className = "brand-mark",
  themeId,
}: {
  className?: string;
  themeId?: string;
}) {
  const theme = brandThemeFor(themeId);
  return (
    <span className={className} aria-hidden="true" data-brand-theme={theme}>
      <img src={brandMarkFor(theme)} alt="" draggable={false} />
    </span>
  );
}
