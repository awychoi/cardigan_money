import makerLogoUrl from "../assets/wychoi-abacus.svg";
import { command } from "../lib/bridge";

/**
 * The maker's website is the only external URL the renderer can ask the app to
 * open. The Rust command hard-codes the address; this component never supplies
 * one. Failures are silent: the address remains readable next to the button.
 */
function MakerLink() {
  return (
    <button
      type="button"
      className="link-button"
      title="Open wychoi.com in your browser"
      onClick={() => {
        void command("open_website", {}).catch(() => undefined);
      }}
    >
      wychoi.com
    </button>
  );
}

/** The personal mark, name and site. Canonical in Settings → About. */
export function MakerCard() {
  return (
    <div className="maker-card">
      <img className="maker-logo" src={makerLogoUrl} alt="" draggable={false} />
      <div>
        <div className="maker-name">Made by Wai Yin Choi</div>
        <div className="maker-meta">
          New York · <MakerLink />
        </div>
      </div>
    </div>
  );
}

/** A one-line credit for the gate and the app footer. */
export function MakerLine({
  className = "",
  withLogo = true,
}: {
  className?: string;
  withLogo?: boolean;
}) {
  return (
    <span className={`maker-line ${className}`.trim()}>
      {withLogo ? (
        <img className="maker-line-logo" src={makerLogoUrl} alt="" draggable={false} />
      ) : null}
      <span>
        Made by Wai Yin Choi · <MakerLink />
      </span>
    </span>
  );
}
