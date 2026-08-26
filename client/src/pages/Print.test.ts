import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";

import Print from "./Print.svelte";

function stateContainer(): HTMLElement {
  const container = document.querySelector<HTMLElement>("[data-state]");
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

describe("Print", () => {
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it("posts the textarea text once, disables the button while printing, then reports success", async () => {
    const pending = deferred<Response>();
    const fetchMock = vi.fn<typeof fetch>().mockReturnValue(pending.promise);
    vi.stubGlobal("fetch", fetchMock);

    render(Print);
    const textarea = screen.getByRole("textbox", { name: "ラベルの文字" });
    const button = screen.getByRole("button", { name: "印刷" });
    expect(stateContainer().dataset.state).toBe("idle");

    await fireEvent.input(textarea, { target: { value: "abc\n12mm テスト" } });
    await fireEvent.click(button);
    await fireEvent.click(button);

    expect(stateContainer().dataset.state).toBe("printing");
    expect((button as HTMLButtonElement).disabled).toBe(true);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/print");
    expect(init?.method).toBe("POST");
    expect(JSON.parse(String(init?.body))).toEqual({
      text: "abc\n12mm テスト",
    });

    pending.resolve(
      new Response(JSON.stringify({ output: "done" }), { status: 200 }),
    );
    await waitFor(() => expect(stateContainer().dataset.state).toBe("success"));
    expect((button as HTMLButtonElement).disabled).toBe(false);
    expect(screen.getByText("印刷しました")).toBeTruthy();
  });

  it("shows the printer's error message when printing fails", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn<typeof fetch>().mockResolvedValue(
        new Response(JSON.stringify({ error: "no printer found" }), {
          status: 502,
        }),
      ),
    );

    render(Print);
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

  it("does not submit blank text", async () => {
    const fetchMock = vi.fn<typeof fetch>();
    vi.stubGlobal("fetch", fetchMock);

    render(Print);
    await fireEvent.click(screen.getByRole("button", { name: "印刷" }));

    expect(fetchMock).not.toHaveBeenCalled();
    expect(stateContainer().dataset.state).toBe("idle");
  });
});
