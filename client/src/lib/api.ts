export type Align = "left" | "center" | "right";

export interface PrintOptions {
  text: string;
  offset_percent: number;
  font: string | null;
  font_scale_percent: number;
  align: Align;
}

export type Connector = "newline" | "space" | "none";

/** One job of several labels; every label shares the settings. */
export interface ContinuousOptions {
  headers: string[];
  bodies: string[];
  connector: Connector;
  offset_percent: number;
  font: string | null;
  font_scale_percent: number;
  align: Align;
}

export interface PrintResult {
  output: string;
}

export interface Preview {
  png_base64: string;
  width_px: number;
  height_px: number;
  tape_px: number;
  length_mm: number;
}

export interface Fonts {
  fonts: string[];
  default: string;
}

export class PrintError extends Error {}

async function postJson<T>(url: string, body: unknown): Promise<T> {
  const response = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  const payload = (await response.json()) as T & { error?: string };
  if (!response.ok) {
    throw new PrintError(payload.error ?? `HTTP ${response.status}`);
  }
  return payload;
}

function body(options: PrintOptions) {
  return {
    text: options.text,
    offset_percent: options.offset_percent,
    font: options.font ?? undefined,
    font_scale_percent: options.font_scale_percent,
    align: options.align,
  };
}

export function postPrint(options: PrintOptions): Promise<PrintResult> {
  return postJson("/api/print", body(options));
}

export function postContinuousPrint(
  options: ContinuousOptions,
): Promise<PrintResult> {
  return postJson("/api/print/continuous", {
    headers: options.headers,
    bodies: options.bodies,
    connector: options.connector,
    offset_percent: options.offset_percent,
    font: options.font ?? undefined,
    font_scale_percent: options.font_scale_percent,
    align: options.align,
  });
}

export function postPreview(
  options: PrintOptions,
  tapeMm: number,
): Promise<Preview> {
  return postJson("/api/preview", { ...body(options), tape_mm: tapeMm });
}

export async function fetchFonts(): Promise<Fonts> {
  const response = await fetch("/api/fonts");
  if (!response.ok) {
    throw new PrintError(`HTTP ${response.status}`);
  }
  return (await response.json()) as Fonts;
}
