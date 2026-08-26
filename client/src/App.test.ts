import { cleanup, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it } from "vitest";

import App from "./App.svelte";

describe("App", () => {
  afterEach(() => {
    cleanup();
  });

  it("keeps the invariant header above the print form", () => {
    render(App);

    const header = screen.getByRole("banner");
    const title = header.querySelector('a[href="/"]');
    expect(title?.textContent).toContain("label-server");
    expect(screen.getByRole("button", { name: "メニュー" })).toBeTruthy();
    expect(header.querySelectorAll("a, button")).toHaveLength(2);

    expect(screen.getByRole("textbox", { name: "ラベルの文字" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "印刷" })).toBeTruthy();
  });
});
