export interface PrintResult {
  output: string;
}

export class PrintError extends Error {}

export async function postPrint(text: string): Promise<PrintResult> {
  const response = await fetch("/api/print", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ text }),
  });
  const payload = (await response.json()) as Partial<PrintResult> & {
    error?: string;
  };
  if (!response.ok) {
    throw new PrintError(payload.error ?? `HTTP ${response.status}`);
  }
  return { output: payload.output ?? "" };
}
