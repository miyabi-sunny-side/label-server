import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import Print from "./Print.svelte";

const FONTS = {
  fonts: ["BIZUDPGothic-Regular", "NotoSansCJK-Regular"],
  default: "NotoSansCJK-Regular",
};
const PREVIEW = {
  png_base64: "iVBORw0KGgo=",
  width_px: 348,
  height_px: 68,
  tape_px: 76,
  length_mm: 49.1,
};

function json(payload: unknown, status = 200): Response {
  return new Response(JSON.stringify(payload), { status });
}

function stateContainer(): HTMLElement {
  const container = document.querySelector<HTMLElement>("form[data-state]");
  if (!container) {
    throw new Error("print form with data-state was not found");
  }
  return container;
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

/** fetch stub answering /api/fonts and /api/preview; print is left to the test. */
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
    if (url === "/api/preview") return Promise.resolve(json(PREVIEW));
    if (url === "/api/print" && onPrint) return Promise.resolve(onPrint(init));
    return Promise.reject(new Error(`unexpected ${url}`));
  });
  vi.stubGlobal("fetch", fetchMock);
  return { fetchMock, calls };
}

/**
 * The fonts arrive behind the 詳細 accordion, so tests that touch a
 * hidden control open it first. Tests that only need the catalog loaded
 * wait on the toggle, which renders immediately.
 */
async function openDetails() {
  await fireEvent.click(await screen.findByRole("button", { name: /詳細/ }));
  return screen.findByRole("combobox", { name: "フォント" });
}

describe("Print", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it("shows only 文字サイズ until 詳細 is opened", async () => {
    stubFetch();
    render(Print);

    const toggle = await screen.findByRole("button", { name: /詳細/ });
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(
      (
        screen.getByRole("spinbutton", {
          name: "文字サイズ (%)",
        }) as HTMLInputElement
      ).value,
    ).toBe("40");
    for (const name of ["フォント", "揃え"]) {
      expect(screen.queryByRole("combobox", { name })).toBeNull();
    }
    for (const name of ["オフセット (%)", "余白 (mm)"]) {
      expect(screen.queryByRole("spinbutton", { name })).toBeNull();
    }

    const select = (await openDetails()) as HTMLSelectElement;
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    await waitFor(() => expect(select.options).toHaveLength(2));
    expect(select.value).toBe("NotoSansCJK-Regular");
    expect(
      (
        screen.getByRole("spinbutton", {
          name: "オフセット (%)",
        }) as HTMLInputElement
      ).value,
    ).toBe("5");
    expect(
      (
        screen.getByRole("spinbutton", {
          name: "余白 (mm)",
        }) as HTMLInputElement
      ).value,
    ).toBe("2");
    expect(
      (screen.getByRole("combobox", { name: "揃え" }) as HTMLSelectElement)
        .value,
    ).toBe("left");
  });

  it("sends the hidden defaults even when 詳細 is never opened", async () => {
    const { calls } = stubFetch(() => json({ output: "printed" }));
    render(Print);
    await screen.findByRole("button", { name: /詳細/ });

    await fireEvent.input(
      screen.getByRole("textbox", { name: "ラベルの文字" }),
      { target: { value: "abc" } },
    );
    await fireEvent.click(screen.getByRole("button", { name: "印刷" }));

    await waitFor(() => expect(stateContainer().dataset.state).toBe("success"));
    expect(calls.find((c) => c.url === "/api/print")?.body).toEqual({
      text: "abc",
      offset_percent: 5,
      font: "NotoSansCJK-Regular",
      font_scale_percent: 40,
      margin_mm: 2,
      align: "left",
    });
  });

  it("still sends every option when the font catalog never answers", async () => {
    const calls: { url: string; body: unknown }[] = [];
    vi.stubGlobal(
      "fetch",
      vi.fn<typeof fetch>().mockImplementation((input, init) => {
        const url = String(input);
        calls.push({
          url,
          body: init?.body ? JSON.parse(String(init.body)) : null,
        });
        // The catalog never resolves, so `font` is still null when the
        // preview and the print go out. The bodies must not lose the key
        // over that race.
        if (url === "/api/fonts") return new Promise<Response>(() => {});
        if (url === "/api/preview") return Promise.resolve(json(PREVIEW));
        return Promise.resolve(json({ output: "printed" }));
      }),
    );
    render(Print);
    await screen.findByRole("button", { name: /詳細/ });

    await fireEvent.input(
      screen.getByRole("textbox", { name: "ラベルの文字" }),
      { target: { value: "abc" } },
    );
    await vi.advanceTimersByTimeAsync(400);
    await waitFor(() =>
      expect(calls.filter((c) => c.url === "/api/preview")).toHaveLength(1),
    );

    await fireEvent.click(screen.getByRole("button", { name: "印刷" }));
    await waitFor(() => expect(stateContainer().dataset.state).toBe("success"));

    const shared = [
      "align",
      "font",
      "font_scale_percent",
      "margin_mm",
      "offset_percent",
      "text",
    ];
    for (const [url, keys] of [
      ["/api/print", shared],
      ["/api/preview", [...shared, "tape_mm"].sort()],
    ] as const) {
      const body = calls.find((c) => c.url === url)?.body as Record<
        string,
        unknown
      >;
      expect(body, url).toBeTruthy();
      expect(Object.keys(body).sort(), url).toEqual(keys);
      expect(body.font, url).toBeNull();
    }
  });

  it("keeps a changed detail after 詳細 is closed again", async () => {
    const { calls } = stubFetch(() => json({ output: "printed" }));
    render(Print);
    const toggle = await screen.findByRole("button", { name: /詳細/ });
    await openDetails();

    await fireEvent.input(
      screen.getByRole("spinbutton", { name: "余白 (mm)" }),
      {
        target: { value: "20" },
      },
    );
    await fireEvent.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByRole("spinbutton", { name: "余白 (mm)" })).toBeNull();

    await fireEvent.input(
      screen.getByRole("textbox", { name: "ラベルの文字" }),
      { target: { value: "abc" } },
    );
    await fireEvent.click(screen.getByRole("button", { name: "印刷" }));

    await waitFor(() => expect(stateContainer().dataset.state).toBe("success"));
    expect(
      (calls.find((c) => c.url === "/api/print")?.body as { margin_mm: number })
        .margin_mm,
    ).toBe(20);
  });

  it("previews the label after typing and reports the tape length", async () => {
    const { calls } = stubFetch();
    render(Print);
    await openDetails();

    await fireEvent.input(
      screen.getByRole("textbox", { name: "ラベルの文字" }),
      {
        target: { value: "Gridfinity" },
      },
    );
    await fireEvent.input(
      screen.getByRole("spinbutton", { name: "オフセット (%)" }),
      {
        target: { value: "10" },
      },
    );
    await vi.advanceTimersByTimeAsync(400);

    await waitFor(() =>
      expect(
        document.querySelector("[data-preview]")?.getAttribute("data-preview"),
      ).toBe("ready"),
    );
    const previews = calls.filter((c) => c.url === "/api/preview");
    expect(previews).toHaveLength(1);
    expect(previews[0].body).toEqual({
      text: "Gridfinity",
      offset_percent: 10,
      font: "NotoSansCJK-Regular",
      font_scale_percent: 40,
      margin_mm: 2,
      align: "left",
      tape_mm: 12,
    });
    const img = screen.getByRole("img", {
      name: "ラベルのプレビュー",
    }) as HTMLImageElement;
    expect(img.src).toBe("data:image/png;base64,iVBORw0KGgo=");
    expect(screen.getByText(/約 49\.1 mm/)).toBeTruthy();
    expect(screen.getByText(/348 px/)).toBeTruthy();
  });

  it("ignores a stale preview response that resolves after a newer one", async () => {
    const first = deferred<Response>();
    const calls: unknown[] = [];
    vi.stubGlobal(
      "fetch",
      vi.fn<typeof fetch>().mockImplementation((input, init) => {
        const url = String(input);
        if (url === "/api/fonts") return Promise.resolve(json(FONTS));
        calls.push(JSON.parse(String(init?.body)));
        // the first preview hangs, the second answers immediately
        return calls.length === 1
          ? first.promise
          : Promise.resolve(
              json({ ...PREVIEW, width_px: 94, length_mm: 13.3 }),
            );
      }),
    );
    render(Print);
    await screen.findByRole("button", { name: /詳細/ });

    const textarea = screen.getByRole("textbox", { name: "ラベルの文字" });
    await fireEvent.input(textarea, { target: { value: "first" } });
    await vi.advanceTimersByTimeAsync(400);
    await fireEvent.input(textarea, { target: { value: "second" } });
    await vi.advanceTimersByTimeAsync(400);
    await waitFor(() => expect(screen.getByText(/約 13\.3 mm/)).toBeTruthy());

    first.resolve(json(PREVIEW));
    await vi.advanceTimersByTimeAsync(10);
    expect(screen.getByText(/約 13\.3 mm/)).toBeTruthy();
    expect(screen.queryByText(/約 49\.1 mm/)).toBeNull();
    expect(calls).toHaveLength(2);
  });

  it("stays idle when the text is cleared while a preview is in flight", async () => {
    const pending = deferred<Response>();
    vi.stubGlobal(
      "fetch",
      vi.fn<typeof fetch>().mockImplementation((input) => {
        if (String(input) === "/api/fonts") return Promise.resolve(json(FONTS));
        return pending.promise;
      }),
    );
    render(Print);
    await screen.findByRole("button", { name: /詳細/ });

    const textarea = screen.getByRole("textbox", { name: "ラベルの文字" });
    await fireEvent.input(textarea, { target: { value: "abc" } });
    await vi.advanceTimersByTimeAsync(400);
    expect(
      document.querySelector("[data-preview]")?.getAttribute("data-preview"),
    ).toBe("loading");

    await fireEvent.input(textarea, { target: { value: "" } });
    pending.resolve(json(PREVIEW));
    await vi.advanceTimersByTimeAsync(10);
    expect(
      document.querySelector("[data-preview]")?.getAttribute("data-preview"),
    ).toBe("idle");
    expect(
      screen.queryByRole("img", { name: "ラベルのプレビュー" }),
    ).toBeNull();
  });

  it("prints with the same options as the preview, once, and reports success", async () => {
    const pending = deferred<Response>();
    const { calls } = stubFetch(() => pending.promise);
    render(Print);
    await openDetails();

    await fireEvent.input(
      screen.getByRole("textbox", { name: "ラベルの文字" }),
      {
        target: { value: "abc\n12mm テスト" },
      },
    );
    await fireEvent.change(screen.getByRole("combobox", { name: "フォント" }), {
      target: { value: "BIZUDPGothic-Regular" },
    });
    await fireEvent.input(
      screen.getByRole("spinbutton", { name: "文字サイズ (%)" }),
      {
        target: { value: "70" },
      },
    );
    await fireEvent.change(screen.getByRole("combobox", { name: "揃え" }), {
      target: { value: "center" },
    });
    const button = screen.getByRole("button", { name: "印刷" });
    await fireEvent.click(button);
    await fireEvent.click(button);

    expect(stateContainer().dataset.state).toBe("printing");
    expect((button as HTMLButtonElement).disabled).toBe(true);
    const prints = calls.filter((c) => c.url === "/api/print");
    expect(prints).toHaveLength(1);
    expect(prints[0].body).toEqual({
      text: "abc\n12mm テスト",
      offset_percent: 5,
      font: "BIZUDPGothic-Regular",
      font_scale_percent: 70,
      margin_mm: 2,
      align: "center",
    });

    pending.resolve(json({ output: "printed" }));
    await waitFor(() => expect(stateContainer().dataset.state).toBe("success"));
    expect(screen.getByText("印刷しました")).toBeTruthy();
  });

  it("shows the printer's error message when printing fails", async () => {
    stubFetch(() => json({ error: "no printer found" }, 502));
    render(Print);
    await screen.findByRole("button", { name: /詳細/ });
    await fireEvent.input(
      screen.getByRole("textbox", { name: "ラベルの文字" }),
      {
        target: { value: "abc" },
      },
    );
    await fireEvent.click(screen.getByRole("button", { name: "印刷" }));

    await waitFor(() => expect(stateContainer().dataset.state).toBe("error"));
    expect(screen.getByText("no printer found")).toBeTruthy();
  });

  it("does not submit or preview blank text", async () => {
    const { calls } = stubFetch();
    render(Print);
    await screen.findByRole("button", { name: /詳細/ });
    await fireEvent.click(screen.getByRole("button", { name: "印刷" }));
    await vi.advanceTimersByTimeAsync(400);

    expect(calls.filter((c) => c.url !== "/api/fonts")).toHaveLength(0);
    expect(stateContainer().dataset.state).toBe("idle");
    expect(
      document.querySelector("[data-preview]")?.getAttribute("data-preview"),
    ).toBe("idle");
  });
});
