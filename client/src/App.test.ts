import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/svelte";
import { afterEach, describe, expect, it } from "vitest";

import App from "./App.svelte";

describe("App", () => {
  afterEach(() => {
    cleanup();
    window.history.pushState(null, "", "/");
  });

  it("keeps the invariant header above the print form", () => {
    render(App);

    const header = screen.getByRole("banner");
    const title = header.querySelector('a.title[href="/"]');
    expect(title?.textContent).toContain("label-server");
    expect(screen.getByRole("button", { name: "メニュー" })).toBeTruthy();
    // title, the two mode tabs and the hamburger
    expect(header.querySelectorAll("a, button")).toHaveLength(4);

    expect(screen.getByRole("textbox", { name: "ラベルの文字" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "印刷" })).toBeTruthy();
  });

  it("routes / to the individual mode and /continuous to the batch one", async () => {
    window.history.pushState(null, "", "/continuous");
    render(App);

    const tabs = screen.getByRole("navigation", { name: "印刷モード" });
    expect(
      tabs.querySelector('[href="/continuous"]')?.getAttribute("aria-current"),
    ).toBe("page");
    expect(
      tabs.querySelector('[href="/"]')?.getAttribute("aria-current"),
    ).toBeNull();
    expect(
      screen.getByRole("textbox", { name: "ヘッダーワード" }),
    ).toBeTruthy();

    await fireEvent.click(tabs.querySelector('[href="/"]') as HTMLElement);

    expect(window.location.pathname).toBe("/");
    expect(
      screen.queryByRole("textbox", { name: "ヘッダーワード" }),
    ).toBeNull();
    expect(screen.getByRole("textbox", { name: "ラベルの文字" })).toBeTruthy();
  });

  it("follows the back button between the two modes", async () => {
    window.history.pushState(null, "", "/");
    render(App);
    expect(screen.getByRole("textbox", { name: "ラベルの文字" })).toBeTruthy();

    await fireEvent.click(
      screen
        .getByRole("navigation", { name: "印刷モード" })
        .querySelector('[href="/continuous"]') as HTMLElement,
    );
    expect(
      screen.getByRole("textbox", { name: "ヘッダーワード" }),
    ).toBeTruthy();

    window.history.back();
    // Both pages caption their textarea ラベルの文字, so the header word
    // input is what tells the two modes apart.
    await waitFor(() =>
      expect(
        screen.queryByRole("textbox", { name: "ヘッダーワード" }),
      ).toBeNull(),
    );
    expect(window.location.pathname).toBe("/");
    expect(screen.getByRole("textbox", { name: "ラベルの文字" })).toBeTruthy();
  });
});
