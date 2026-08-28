import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import Continuous from "./Continuous.svelte";

const FONTS = {
  fonts: ["BIZUDPGothic-Regular", "NotoSansCJK-Regular"],
  default: "NotoSansCJK-Regular",
};

function json(payload: unknown, status = 200): Response {
  return new Response(JSON.stringify(payload), { status });
}

function stateContainer(): HTMLElement {
  const container = document.querySelector<HTMLElement>("form[data-state]");
  if (!container) {
    throw new Error("continuous form with data-state was not found");
  }
  return container;
}

/** fetch stub answering /api/fonts; the print call is left to the test. */
function stubFetch(
  onPrint?: (init: RequestInit | undefined) => Response | Promise<Response>,
) {
  const calls: { url: string; body: unknown }[] = [];
  const fetchMock = vi.fn<typeof fetch>().mockImplementation((input, init) => {
    const url = String(input);
    calls.push({
      url,
      body: init?.body ? JSON.parse(String(init.body)) : null,
    });
    if (url === "/api/fonts") return Promise.resolve(json(FONTS));
    if (url === "/api/print/continuous" && onPrint) {
      return Promise.resolve(onPrint(init));
    }
    return Promise.reject(new Error(`unexpected ${url}`));
  });
  vi.stubGlobal("fetch", fetchMock);
  return calls;
}

async function fill(header: string, text: string) {
  await fireEvent.input(
    screen.getByRole("textbox", { name: "ヘッダーワード" }),
    {
      target: { value: header },
    },
  );
  await fireEvent.input(screen.getByRole("textbox", { name: "ラベルの文字" }), {
    target: { value: text },
  });
}

describe("Continuous page", () => {
  beforeEach(() => {
    vi.useRealTimers();
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it("sends one request carrying every label and the shared settings", async () => {
    const calls = stubFetch(() => json({ output: "printed 3 labels" }));
    render(Continuous);
    await waitFor(() =>
      expect(calls.some((c) => c.url === "/api/fonts")).toBe(true),
    );

    await fill("M4", "皿8\n皿10\n小ト12");
    await fireEvent.click(screen.getByRole("button", { name: "印刷" }));

    await waitFor(() => expect(stateContainer().dataset.state).toBe("success"));
    const prints = calls.filter((c) => c.url === "/api/print/continuous");
    expect(prints).toHaveLength(1);
    expect(prints[0].body).toEqual({
      headers: ["M4", "M4", "M4"],
      bodies: ["皿8", "皿10", "小ト12"],
      connector: "space",
      offset_percent: 5,
      font: "NotoSansCJK-Regular",
      font_scale_percent: 100,
      align: "left",
    });
  });

  it("counts the labels and refuses to print none", async () => {
    stubFetch();
    render(Continuous);

    const button = screen.getByRole("button", { name: "印刷" });
    expect(screen.getByText("0 枚")).toBeTruthy();
    expect(button.hasAttribute("disabled")).toBe(true);

    await fill("", "皿8\n\n  \n皿10\n");

    // Blank lines make no label, so they are not counted either.
    expect(screen.getByText("2 枚")).toBeTruthy();
    expect(button.hasAttribute("disabled")).toBe(false);
  });

  it("passes the chosen connector through", async () => {
    const calls = stubFetch(() => json({ output: "printed 1 label" }));
    render(Continuous);

    await fill("M4", "皿8");
    await fireEvent.change(
      screen.getByRole("combobox", { name: "接続ワード" }),
      {
        target: { value: "newline" },
      },
    );
    await fireEvent.click(screen.getByRole("button", { name: "印刷" }));

    await waitFor(() => expect(stateContainer().dataset.state).toBe("success"));
    const print = calls.find((c) => c.url === "/api/print/continuous");
    expect((print?.body as { connector: string }).connector).toBe("newline");
  });

  it("shows the printer's own message when the job fails", async () => {
    stubFetch(() => json({ error: "no PT-P700 found on USB" }, 502));
    render(Continuous);

    await fill("M4", "皿8");
    await fireEvent.click(screen.getByRole("button", { name: "印刷" }));

    await waitFor(() => expect(stateContainer().dataset.state).toBe("error"));
    expect(screen.getByRole("alert").textContent).toContain(
      "no PT-P700 found on USB",
    );
  });
});
