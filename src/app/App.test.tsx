import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";
import { __resetMockBridge, __setMockVaultState } from "../mocks/bridge";

async function unlock() {
  const user = userEvent.setup();
  render(<App />);
  expect(await screen.findByText(/Your encrypted ledger is locked/)).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Unlock vault" }));
  expect(await screen.findByText("Your cards, in one quiet place.")).toBeInTheDocument();
  return user;
}

describe("app shell", () => {
  beforeEach(() => {
    window.localStorage.clear();
    __resetMockBridge();
    __setMockVaultState({ vaultExists: true, unlocked: false });
  });

  it("starts locked with no password input and a native-sheet explanation", async () => {
    render(<App />);
    expect(await screen.findByText(/Your encrypted ledger is locked/)).toBeInTheDocument();
    expect(document.querySelector("input[type='password']")).toBeNull();
    expect(screen.getByText(/native macOS sheet/i)).toBeInTheDocument();
    expect(screen.getByText("cardigan_money")).toBeInTheDocument();
  });

  it("offers vault creation when no vault exists", async () => {
    __setMockVaultState({ vaultExists: false, unlocked: false });
    const user = userEvent.setup();
    render(<App />);
    expect(await screen.findByText(/Create your encrypted ledger/)).toBeInTheDocument();
    expect(screen.getByText(/no recovery if you lose it/i)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /Create vault/ }));
    expect(await screen.findByText("Your cards, in one quiet place.")).toBeInTheDocument();
  });

  it("unlocks through the typed bridge and shows the secure shell", async () => {
    await unlock();
    expect(screen.getByText(/No provider connected/)).toBeInTheDocument();
    expect(screen.getByText(/Mock desktop bridge/)).toBeInTheDocument();
  });

  it("locks again and clears the unlocked view", async () => {
    const user = await unlock();
    await user.click(screen.getByRole("button", { name: "Lock now" }));
    expect(await screen.findByText(/Your encrypted ledger is locked/)).toBeInTheDocument();
    expect(screen.queryByText("Your cards, in one quiet place.")).not.toBeInTheDocument();
  });

  it("stores no financial data or theme in browser storage", async () => {
    await unlock();
    expect(Object.keys(window.localStorage)).toEqual([]);
    expect(Object.keys(window.sessionStorage)).toEqual([]);
    expect(window.location.href).not.toMatch(/passphrase|token|secret/i);
  });

  it("toggles appearance through encrypted settings", async () => {
    const user = await unlock();
    const toggle = screen.getByRole("button", { name: /appearance/i });
    await user.click(toggle);
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("light"));
    expect(Object.keys(window.localStorage)).toEqual([]);
  });

  it("shows real cipher, KDF, and security events in settings", async () => {
    const user = await unlock();
    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(await screen.findByText("SQLCipher")).toBeInTheDocument();
    expect(screen.getAllByText(/4\.6\.1 community/).length).toBeGreaterThan(0);
    expect(screen.getByText(/Argon2id/)).toBeInTheDocument();
    expect((await screen.findAllByText(/unlock success/i)).length).toBeGreaterThan(0);
    expect(document.querySelector("input[type='password']")).toBeNull();
    expect(document.body.textContent ?? "").not.toContain("correct horse");
    expect(document.body.textContent ?? "").not.toContain("sentinel");
  });
});
