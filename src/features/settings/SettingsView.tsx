import { useEffect, useState } from "react";
import {
  command,
  type SecurityEvent,
  type Settings,
  type SettingsPatch,
  type VaultState,
} from "../../lib/bridge";
import { friendlyError } from "../../lib/bridge/errors";
import { Button } from "../../components/Button";
import { ThemeToggle } from "../../components/ThemeToggle";
import { AccountManager } from "../accounts/AccountManager";
import { MerchantRulesPanel } from "./MerchantRulesPanel";
import { ManualPaymentPanel } from "./ManualPaymentPanel";
import { ImportExportPanel } from "./ImportExportPanel";
import { BackupsPanel } from "./BackupsPanel";

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
}) {
  const settings = vault.settings;
  const [events, setEvents] = useState<SecurityEvent[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [timezone, setTimezone] = useState(settings?.timezone ?? "America/New_York");
  const [autoLock, setAutoLock] = useState(settings?.autoLockMinutes ?? 15);
  const [busy, setBusy] = useState(false);

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

      <div className="settings-grid">
        <section className="panel" aria-labelledby="appearance-title">
          <h2 id="appearance-title">Appearance</h2>
          <div className="setting-row">
            <div>
              <div className="setting-label">Theme</div>
              <p className="fine-print">Follow the system or pin light or dark.</p>
            </div>
            <ThemeToggle theme={settings.theme} onChange={(theme) => update({ theme })} />
          </div>
        </section>

        <section className="panel" aria-labelledby="lock-title">
          <h2 id="lock-title">Locking</h2>
          <div className="setting-row">
            <div>
              <div className="setting-label">Idle auto-lock</div>
              <p className="fine-print">
                The vault locks in Rust after this much system-wide inactivity. Screen lock and sleep
                always lock.
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
        </section>

        <AccountManager onChanged={onAccountsChanged} readOnly={demo} />

        {!demo ? (
          <>
            <BackupsPanel
              onDataChanged={onDataChanged}
              onVaultChanged={onVaultChanged}
              lastBackupAt={settings.lastBackupAt}
              lastBackupStatus={settings.lastBackupStatus}
            />
            <ManualPaymentPanel onChanged={onDataChanged} />
            <MerchantRulesPanel onChanged={onDataChanged} />
            <ImportExportPanel onChanged={onDataChanged} />
          </>
        ) : null}

        <section className="panel" aria-labelledby="close-title">
          <h2 id="close-title">Window close behavior</h2>
          <div className="setting-row">
            <div>
              <div className="setting-label">When the window closes</div>
              <p className="fine-print">
                Hiding is not a lock: the app and scheduler keep running, the idle timer continues,
                and screen lock or sleep still locks the vault.
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
        </section>

        <section className="panel" aria-labelledby="timezone-title">
          <h2 id="timezone-title">Calendar &amp; timezone</h2>
          <div className="setting-row">
            <div>
              <div className="setting-label">Reporting timezone</div>
              <p className="fine-print">Calendar months are evaluated in this timezone.</p>
            </div>
            <input
              aria-label="Reporting timezone"
              value={timezone}
              onChange={(event) => setTimezone(event.target.value)}
              spellCheck={false}
              disabled={busy}
              className="text-input"
            />
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
        </section>

        <section className="panel" aria-labelledby="security-title">
          <h2 id="security-title">Security</h2>
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
            Changing the passphrase rotates the data encryption key. Older backups remain readable
            only with the old passphrase. There is no recovery if every passphrase is lost.
          </p>
        </section>

        <section className="panel" aria-labelledby="demo-title">
          <h2 id="demo-title">Demo mode</h2>
          <p className="fine-print">
            Demo mode is a separate database with synthetic cards for exploring the dashboard. It
            cannot open the real ledger, read the Keychain, import real CSV, or create backups or
            exports. Opening demo closes the real session; you will need your passphrase again.
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
        </section>

        <section className="panel" aria-labelledby="quit-title">
          <h2 id="quit-title">Quit</h2>
          <p className="fine-print">
            Quitting closes the database, zeroizes the session, and stops scheduled syncing. The
            passphrase is required again at next launch.
          </p>
          <div className="button-row">
            <Button variant="secondary" onClick={quitApp} disabled={busy}>
              Quit cardigan_money
            </Button>
          </div>
        </section>

        <section className="panel panel-wide" aria-labelledby="events-title">
          <h2 id="events-title">Recent security events</h2>
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
      </div>
    </section>
  );
}
