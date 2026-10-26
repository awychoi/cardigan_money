import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { RestorePreview, RestoreResult } from "../../lib/bridge";

const { command } = vi.hoisted(() => ({ command: vi.fn() }));

vi.mock("../../lib/bridge", () => ({ command }));

import { RestoreBackupFlow } from "./RestoreBackupFlow";

const preview: RestorePreview = {
  backup: {
    id: "cm-backup-manual-5a17c0de-gen1-20261023T120000Z-0000.cmbak",
    kind: "manual",
    createdAt: "2026-10-23T12:00:00Z",
    sizeBytes: 4096,
    generation: 1,
    schemaVersion: 4,
  },
  backupVaultPrefix: "5a17c0de",
  currentVaultPrefix: "deadbeef",
  freshInstall: false,
  sameVault: false,
  requiresTypedConfirmation: true,
  requiresCurrentUnlock: false,
  destructiveAvailable: true,
  requiresPlaidChoice: true,
  nonPrivate: false,
};

const result: RestoreResult = {
  restored: true,
  kind: "manual",
  preRestoreBackupId: "cm-backup-pre-restore-5a17c0de-gen1-20261023T120000Z-0000.cmbak",
  scope: "full",
  vaultIdChanged: true,
  adoptedBackupKey: true,
  retiredBackups: 1,
  vaultKeyChanged: false,
  preRestoreProtection: "previous",
  unlocked: true,
  reusedConnections: 1,
  newLinkConnections: 0,
};

function renderFlow(onRestored = vi.fn()) {
  render(<RestoreBackupFlow onRestored={onRestored} onCancel={vi.fn()} />);
  return onRestored;
}

describe("RestoreBackupFlow", () => {
  beforeEach(() => {
    command.mockReset();
    command.mockImplementation((name: string) =>
      Promise.resolve(name === "backup_restore_prepare" ? preview : result),
    );
  });

  it("offers reuse of local Plaid Items by default for a cross-vault restore", async () => {
    const user = userEvent.setup();
    const onRestored = renderFlow();

    expect(await screen.findByText("Restore a backup from a different vault?")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: /Reuse the Plaid Items still stored/ })).toBeChecked();

    await user.type(screen.getByLabelText("Type replace to confirm"), "replace");
    await user.click(screen.getByRole("button", { name: "Restore this backup…" }));

    expect(command).toHaveBeenCalledWith(
      "backup_restore_apply",
      expect.objectContaining({
        scope: "full",
        confirmation: "replace",
        plaidChoice: "reuse-local",
      }),
    );
    expect(onRestored).toHaveBeenCalledWith(result);
  });

  it("sends the delete-and-revoke choice when selected", async () => {
    const user = userEvent.setup();
    renderFlow();

    await user.click(
      await screen.findByRole("radio", {
        name: /Delete local tokens and revoke the old Plaid Items remotely/,
      }),
    );
    await user.type(screen.getByLabelText("Type replace to confirm"), "replace");
    await user.click(screen.getByRole("button", { name: "Restore this backup…" }));

    expect(command).toHaveBeenCalledWith(
      "backup_restore_apply",
      expect.objectContaining({ plaidChoice: "delete-and-revoke" }),
    );
  });
});
