import markUrl from "../assets/brand-mark.svg";

/**
 * The cardigan_money mark: the selected Forest button, cropped to its macOS
 * tile so it fills the slot at every size (sidebar, vault, boot screen).
 */
export function BrandMark({ className = "brand-mark" }: { className?: string }) {
  return (
    <span className={className} aria-hidden="true">
      <img src={markUrl} alt="" draggable={false} />
    </span>
  );
}
