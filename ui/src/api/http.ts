/**
 * Thin fetch wrapper.
 *
 * The backend returns errors as plain text bodies with a status code
 * (see `ApiError::into_response` in app/src/web_api/mod.rs), and most mutating
 * endpoints answer with an empty 200. Both are handled here so call sites do not
 * have to think about it.
 */

export const API_BASE = import.meta.env.DEV
  ? (import.meta.env.VITE_API_BASE ?? "http://localhost:3000")
  : window.location.origin;

export class ApiError extends Error {
  readonly status: number;

  constructor(status: number, message: string) {
    super(message || `Request failed with status ${status}`);
    this.name = "ApiError";
    this.status = status;
  }
}

interface RequestOptions {
  method?: "GET" | "POST" | "PUT" | "DELETE";
  body?: unknown;
  signal?: AbortSignal;
}

async function send(path: string, options: RequestOptions = {}): Promise<Response> {
  const { method = "GET", body, signal } = options;

  const res = await fetch(`${API_BASE}${path}`, {
    method,
    signal,
    headers: body === undefined ? undefined : { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });

  if (!res.ok) {
    // Error bodies are plain text, not JSON.
    const text = await res.text().catch(() => "");
    throw new ApiError(res.status, text.trim());
  }

  return res;
}

/** Perform a request and parse the JSON body. */
export async function requestJson<T>(path: string, options?: RequestOptions): Promise<T> {
  const res = await send(path, options);
  return (await res.json()) as T;
}

/** Perform a request that returns an empty body. */
export async function requestEmpty(path: string, options?: RequestOptions): Promise<void> {
  await send(path, options);
}

/** Perform a request and return the raw text body. */
export async function requestText(path: string, options?: RequestOptions): Promise<string> {
  const res = await send(path, options);
  return await res.text();
}

/** Perform a request and return the status code, for endpoints where it is meaningful. */
export async function requestStatus(path: string, options?: RequestOptions): Promise<number> {
  const res = await send(path, options);
  return res.status;
}
