import { useEffect, useMemo, useState, type KeyboardEvent } from "react";
import {
  command,
  type SecurityEvent,
  type Settings,
  type SettingsPatch,
  type VaultState,
} from "../../lib/bridge";
import { friendlyError } from "../../lib/bridge/errors";
import { Button } from "../../components/Button";
import { ThemePicker } from "../../components/ThemePicker";
import { TimezoneSelect } from "../../components/TimezoneSelect";
import {
  detectTimeZone,
  FALLBACK_TIMEZONE,
  formatTimeZonePreview,
  timeZoneSummary,
} from "../../lib/timezone";
import type { ThemeMode } from "../../theme/schema";
import { AccountManager } from "../accounts/AccountManager";
import { MerchantRulesPanel } from "./MerchantRulesPanel";
import { ImportExportPanel } from "./ImportExportPanel";
import { BackupsPanel } from "./BackupsPanel";
import { ConnectionsSyncPanel } from "./ConnectionsSyncPanel";

export type SettingsSectionId =
  | "appearance"
  | "lock"
  | "accounts"
  | "backups"
  | "rules"
  | "events"
  | "about";

export type AccountsTabId = "accounts" | "connections" | "import";

const ACCOUNTS_TABS: { id: AccountsTabId; label: string }[] = [
  { id: "accounts", label: "All accounts" },
  { id: "connections", label: "Bank connections" },
  { id: "import", label: "CSV import" },
];

export function SettingsView({
  vault,
  onSettingsChanged,
  onLock,
  onAccountsChanged,
  onDataChanged,
  onVaultChanged,
  demo,
  onOpenDemo,
  onResetDemo,
  initialSection,
  initialAccountsTab,
}: {
  vault: VaultState;
  onSettingsChanged: (settings: Settings) => void;
  onLock: () => Promise<void>;
  onAccountsChanged: () => void;
  onDataChanged: () => void;
  onVaultChanged: (vault: VaultState) => void;
  demo: boolean;
  onOpenDemo: () => Promise<void>;
  onResetDemo: () => Promise<void>;
  initialSection?: SettingsSectionId;
  initialAccountsTab?: AccountsTabId;
}) {
  const settings = vault.settings;
  const previewMode: ThemeMode =
    document.documentElement.dataset.theme === "dark" ? "dark" : "light";
  const [events, setEvents] = useState<SecurityEvent[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [timezone, setTimezone] = useState(settings?.timezone ?? FALLBACK_TIMEZONE);
  const [autoLock, setAutoLock] = useState(settings?.autoLockMinutes ?? 15);
  const [busy, setBusy] = useState(false);
  const [section, setSection] = useState<SettingsSectionId>(initialSection ?? "appearance");
  const [accountsTab, setAccountsTab] = useState<AccountsTabId>(initialAccountsTab ?? "accounts");
  const detected = useMemo(() => detectTimeZone(), []);
  const timezonePreview = formatTimeZonePreview(timezone);

  useEffect(() => {
    if (settings?.timezone) setTimezone(settings.timezone);
  }, [settings?.timezone]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const result = await command("security_events_list", { limit: 12 });
        if (!cancelled) setEvents(result.events);
      } catch {
        if (!cancelled) setEvents([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  if (!settings) {
    return (
      <section className="panel" aria-labelledby="settings-title">
        <h1 id="settings-title">Settings</h1>
        <p className="muted">Unlock the vault to view settings.</p>
      </section>
    );
  }

  async function update(patch: SettingsPatch) {
    setBusy(true);
    setMessage(null);
    setError(null);
    try {
      const result = await command("settings_update", patch);
      onSettingsChanged(result.settings);
      setMessage("Saved.");
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  async function quitApp() {
    setBusy(true);
    setError(null);
    try {
      await command("app_quit", {});
    } catch (caught) {
      setError(friendlyError(caught));
      setBusy(false);
    }
  }

  async function changePassphrase() {
    setBusy(true);
    setMessage(null);
    setError(null);
    try {
      await command("vault_change_passphrase", {});
      setMessage("Passphrase changed. Older backups still require the old passphrase.");
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  function handleAccountsTabKey(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const last = accountsTabs.length - 1;
    const nextIndex =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? last
          : event.key === "ArrowLeft"
            ? (index - 1 + accountsTabs.length) % accountsTabs.length
            : (index + 1) % accountsTabs.length;
    const next = accountsTabs[nextIndex];
    if (!next) return;
    setAccountsTab(next.id);
    document.getElementById(`accounts-tab-${next.id}`)?.focus();
  }

  const sections: Array<{ id: SettingsSectionId; label: string }> = [
    { id: "appearance", label: "Appearance" },
    { id: "lock", label: "Lock & idle" },
    { id: "accounts", label: "Accounts" },
    ...(demo
      ? []
      : ([
          { id: "backups", label: "Backups & recovery" },
          { id: "rules", label: "Merchant rules" },
        ] as const)),
    { id: "events", label: "Security events" },
    { id: "about", label: "About" },
  ];
  const active = sections.some((item) => item.id === section) ? section : "appearance";
  const accountsTabs = demo ? ACCOUNTS_TABS.slice(0, 1) : ACCOUNTS_TABS;
  const activeAccountsTab = accountsTabs.some((tab) => tab.id === accountsTab)
    ? accountsTab
    : "accounts";

  return (
    <section className="settings" aria-labelledby="settings-title">
      <header className="page-heading">
        <div>
          <h1 id="settings-title">Settings</h1>
          <p className="muted">Local preferences are stored inside the encrypted database.</p>
        </div>
        <Button onClick={onLock} disabled={busy}>
          Lock now
        </Button>
      </header>

      {message ? (
        <p className="inline-notice" role="status">
          {message}
        </p>
      ) : null}
      {error ? (
        <p className="inline-error" role="alert">
          {error}
        </p>
      ) : null}

      <div className="settings-layout">
        <nav className="settings-nav" aria-label="Settings sections">
          {sections.map((item) => (
            <button
              key={item.id}
              type="button"
              className={`settings-nav-item${
                active === item.id ? " settings-nav-item-active" : ""
              }`}
              aria-current={active === item.id ? "page" : undefined}
              onClick={() => setSection(item.id)}
            >
              {item.label}
            </button>
          ))}
        </nav>

        <div className="settings-section">
          {active === "appearance" ? (
            <section className="panel panel-wide" aria-labelledby="appearance-title">
              <h2 id="appearance-title">Appearance</h2>
              <div className="setting-row">
                <div>
                  <div className="setting-label">Mode</div>
                  <p className="fine-print">
                    Follow the system or pin light or dark. Every theme provides both modes.
                  </p>
                </div>
                <div className="mode-seg" role="group" aria-label="Appearance mode">
                  {(["light", "system", "dark"] as const).map((mode) => (
                    <button
                      key={mode}
                      type="button"
                      aria-pressed={settings.theme === mode}
                      onClick={() => void update({ theme: mode })}
                    >
                      {mode === "light" ? "Light" : mode === "dark" ? "Dark" : "System"}
                    </button>
                  ))}
                </div>
              </div>
              <div className="hr" />
              <ThemePicker
                value={settings.themeId}
                mode={previewMode}
                onChange={(themeId) => void update({ themeId })}
              />
            </section>
          ) : null}

          {active === "lock" ? (
            <section className="panel panel-wide" aria-labelledby="lock-title">
              <h2 id="lock-title">Lock &amp; idle</h2>
              <div className="setting-row">
                <div>
                  <div className="setting-label">Idle auto-lock</div>
                  <p className="fine-print">
                    The vault locks in Rust after this much system-wide inactivity. Screen lock and
                    sleep always lock.
                  </p>
                </div>
                <select
                  aria-label="Idle auto-lock minutes"
                  value={autoLock}
                  onChange={(event) => setAutoLock(Number(event.target.value))}
                  disabled={busy}
                >
                  {[1, 5, 15, 30, 60, 120, 240].map((minutes) => (
                    <option key={minutes} value={minutes}>
                      {minutes} min
                    </option>
                  ))}
                </select>
              </div>
              <div className="setting-row">
                <Button
                  variant="secondary"
                  disabled={busy || autoLock === settings.autoLockMinutes}
                  onClick={() => update({ autoLockMinutes: autoLock })}
                >
                  Apply timeout
                </Button>
              </div>
              <div className="setting-row">
                <div>
                  <div className="setting-label">Lock on system events</div>
                  <p className="fine-print">
                    Screen lock, sleep, fast user switching, logout, and quit. Always enforced.
                  </p>
                </div>
                <span className="status-pill status-ok">Enforced</span>
              </div>

              <div className="hr" />
              <h3 className="settings-subhead">Window close behavior</h3>
              <div className="setting-row">
                <div>
                  <div className="setting-label">When the window closes</div>
                  <p className="fine-print">
                    Hiding is not a lock: the app and scheduler keep running, the idle timer
                    continues, and screen lock or sleep still locks the vault.
                  </p>
                </div>
                <select
                  aria-label="When the window closes"
                  value={settings.closeBehavior}
                  onChange={(event) =>
                    void update({
                      closeBehavior: event.target.value as Settings["closeBehavior"],
                    })
                  }
                  disabled={busy}
                >
                  <option value="unset">Ask each time</option>
                  <option value="hide">Hide</option>
                  <option value="quit">Quit</option>
                </select>
              </div>

              <div className="hr" />
              <h3 className="settings-subhead">Calendar &amp; timezone</h3>
              <div className="setting-row">
                <div>
                  <div className="setting-label">Reporting timezone</div>
                  <p className="fine-print">
                    Calendar months are evaluated in this timezone. Choose the place your statements
                    are based.
                  </p>
                </div>
                <TimezoneSelect
                  value={timezone}
                  onChange={setTimezone}
                  disabled={busy}
                  className="timezone-select-wide"
                />
              </div>
              <div className="setting-row setting-row-stack">
                <p className="fine-print">
                  {timezonePreview ? `Current time there: ${timezonePreview}. ` : ""}
                  {detected.detected ? (
                    <>
                      This Mac reports <strong>{timeZoneSummary(detected.zone)}</strong>.
                    </>
                  ) : (
                    <>
                      We could not detect this Mac&apos;s timezone, so the default is{" "}
                      <strong>{timeZoneSummary(FALLBACK_TIMEZONE)}</strong>.
                    </>
                  )}
                </p>
                {detected.zone !== timezone ? (
                  <Button
                    variant="ghost"
                    className="btn-small"
                    onClick={() => setTimezone(detected.zone)}
                    disabled={busy}
                  >
                    Use detected timezone
                  </Button>
                ) : null}
              </div>
              <div className="setting-row">
                <Button
                  variant="secondary"
                  disabled={busy || timezone === settings.timezone}
                  onClick={() => update({ timezone })}
                >
                  Save timezone
                </Button>
              </div>

              <div className="hr" />
              <h3 className="settings-subhead">Passphrase</h3>
              {!demo ? (
                <div className="button-row">
                  <Button variant="secondary" disabled={busy} onClick={changePassphrase}>
                    Change passphrase…
                  </Button>
                </div>
              ) : (
                <p className="fine-print">Passphrase change is unavailable in demo mode.</p>
              )}
              <p className="fine-print">
                Changing the passphrase re-encrypts the ledger. Older backups still need the old
                passphrase, and there is no recovery if every passphrase is lost.
              </p>
            </section>
          ) : null}

          {active === "accounts" ? (
            <section className="accounts-hub" aria-labelledby="accounts-hub-title">
              <div className="accounts-hub-head">
                <div>
                  <h2 id="accounts-hub-title">Accounts &amp; data</h2>
                  <p className="fine-print">Every card is labeled with where its data came from.</p>
                </div>
              </div>
              <div className="settings-tabs" role="tablist" aria-label="Accounts and data">
                {accountsTabs.map((tab, index) => (
                  <button
                    key={tab.id}
                    id={`accounts-tab-${tab.id}`}
                    type="button"
                    role="tab"
                    aria-selected={activeAccountsTab === tab.id}
                    aria-controls={`accounts-panel-${tab.id}`}
                    tabIndex={activeAccountsTab === tab.id ? 0 : -1}
                    className={`settings-tab${
                      activeAccountsTab === tab.id ? " settings-tab-active" : ""
                    }`}
                    onClick={() => setAccountsTab(tab.id)}
                    onKeyDown={(event) => handleAccountsTabKey(event, index)}
                  >
                    {tab.label}
                  </button>
                ))}
              </div>
              <div
                id={`accounts-panel-${activeAccountsTab}`}
                role="tabpanel"
                aria-labelledby={`accounts-tab-${activeAccountsTab}`}
                className="accounts-tab-panel"
              >
                {activeAccountsTab === "accounts" ? (
                  <AccountManager onChanged={onAccountsChanged} readOnly={demo} />
                ) : null}
                {activeAccountsTab === "connections" && !demo ? (
                  <ConnectionsSyncPanel onSyncFinished={onDataChanged} readOnly={false} />
                ) : null}
                {activeAccountsTab === "import" && !demo ? (
                  <ImportExportPanel onChanged={onDataChanged} />
                ) : null}
              </div>
              {demo ? (
                <p className="fine-print">
                  Bank connections and CSV import are unavailable in demo mode.
                </p>
              ) : null}
            </section>
          ) : null}

          {active === "backups" && !demo ? (
            <BackupsPanel
              onDataChanged={onDataChanged}
              onVaultChanged={onVaultChanged}
              lastBackupAt={settings.lastBackupAt}
              lastBackupStatus={settings.lastBackupStatus}
            />
          ) : null}

          {active === "rules" && !demo ? <MerchantRulesPanel onChanged={onDataChanged} /> : null}

          {active === "events" ? (
            <section className="panel panel-wide" aria-labelledby="events-title">
              <h2 id="events-title">Security events</h2>
              {events.length === 0 ? (
                <p className="muted">No security events recorded yet.</p>
              ) : (
                <table className="events-table">
                  <caption className="sr-only">Recent security events</caption>
                  <thead>
                    <tr>
                      <th scope="col">Time</th>
                      <th scope="col">Event</th>
                      <th scope="col">Outcome</th>
                    </tr>
                  </thead>
                  <tbody>
                    {events.map((event) => (
                      <tr key={event.id}>
                        <td>{new Date(event.createdAt).toLocaleString()}</td>
                        <td>{event.eventType.replaceAll("_", " ")}</td>
                        <td>
                          <span
                            className={`status-pill ${
                              event.outcome === "success"
                                ? "status-ok"
                                : event.outcome === "failure"
                                  ? "status-fail"
                                  : "status-info"
                            }`}
                          >
                            {event.outcome}
                          </span>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
              <p className="fine-print">
                Security events contain no secrets, tokens, account numbers, or financial payloads.
              </p>
            </section>
          ) : null}

          {active === "about" ? (
            <section className="panel panel-wide" aria-labelledby="about-title">
              <h2 id="about-title">About</h2>
              <p className="fine-print">
                cardigan_money is a local-first, single-user encrypted ledger. There is no server
                component and no telemetry.
              </p>
              {vault.security ? (
                <dl className="facts">
                  <div>
                    <dt>SQLCipher</dt>
                    <dd>{vault.security.cipherVersion}</dd>
                  </div>
                  <div>
                    <dt>Provider</dt>
                    <dd>{vault.security.cipherProvider}</dd>
                  </div>
                  <div>
                    <dt>Key derivation</dt>
                    <dd>{vault.security.kdfSummary}</dd>
                  </div>
                  <div>
                    <dt>Key generation</dt>
                    <dd>{vault.security.generation}</dd>
                  </div>
                  <div>
                    <dt>Vault ID</dt>
                    <dd>
                      <code>{vault.security.vaultIdPrefix}…</code>
                    </dd>
                  </div>
                  <div>
                    <dt>Last integrity check</dt>
                    <dd>{vault.security.lastIntegrityCheckAt ?? "Unknown"}</dd>
                  </div>
                </dl>
              ) : (
                <p className="muted">Security status is unavailable.</p>
              )}

              <div className="hr" />
              <h3 className="settings-subhead">Demo mode</h3>
              <p className="fine-print">
                A separate database with synthetic cards; it cannot reach your real ledger. Opening
                demo ends the real session, and your passphrase is needed again.
              </p>
              <div className="button-row">
                {demo ? (
                  <>
                    <Button variant="secondary" onClick={() => void onResetDemo()} disabled={busy}>
                      Reset demo data
                    </Button>
                    <Button variant="ghost" onClick={() => void onOpenDemo()} disabled={busy}>
                      Reopen demo
                    </Button>
                  </>
                ) : (
                  <Button variant="secondary" onClick={() => void onOpenDemo()} disabled={busy}>
                    Open demo (synthetic)…
                  </Button>
                )}
              </div>

              <div className="hr" />
              <h3 className="settings-subhead">Quit</h3>
              <p className="fine-print">
                Closes the database and stops scheduled syncing. Your passphrase is needed at next
                launch.
              </p>
              <div className="button-row">
                <Button variant="secondary" onClick={quitApp} disabled={busy}>
                  Quit cardigan_money
                </Button>
              </div>
            </section>
          ) : null}
        </div>
      </div>
    </section>
  );
}
