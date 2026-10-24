import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";

describe("app shell", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("shows the exact product name and the secure shell", async () => {
    render(<App />);
    expect(await screen.findByText("Your cards, in one quiet place.")).toBeInTheDocument();
    expect(screen.getAllByText("cardigan_money").length).toBeGreaterThan(0);
    expect(screen.getByText(/No provider connected/)).toBeInTheDocument();
    expect(screen.getByText(/Mock desktop bridge/)).toBeInTheDocument();
  });

  it("stores only the non-financial theme preference", async () => {
    render(<App />);
    await screen.findByRole("heading", { level: 1 });
    expect(Object.keys(window.localStorage)).toEqual(["cm.theme"]);
  });

  it("toggles appearance and persists the choice", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { level: 1 });
    const toggle = screen.getByRole("button", { name: /appearance/i });
    await user.click(toggle);
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("light"));
    expect(window.localStorage.getItem("cm.theme")).toBe("light");
  });

  it("hydrates from the typed bridge before showing the shell", async () => {
    const { command } = await import("../lib/bridge");
    await expect(command("app_info", {})).resolves.toBeTruthy();
    render(<App />);
    expect(await screen.findByText(/No provider connected/)).toBeInTheDocument();
  });
});
