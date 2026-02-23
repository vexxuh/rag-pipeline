import type { ApiError } from "$types/index";

const BASE_URL = import.meta.env.VITE_API_URL || "http://localhost:3000";

function getToken(): string | null {
  if (typeof window === "undefined") return null;
  return localStorage.getItem("auth_token");
}

async function request<T>(
  endpoint: string,
  options: RequestInit = {},
): Promise<T> {
  const token = getToken();

  const headers: HeadersInit = {
    "Content-Type": "application/json",
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
    ...(options.headers || {}),
  };

  let response: Response;
  try {
    response = await fetch(`${BASE_URL}${endpoint}`, {
      ...options,
      headers,
    });
  } catch {
    throw new Error(
      "Cannot connect to the server. Make sure the backend is running.",
    );
  }

  if (!response.ok) {
    const isAuthEndpoint = endpoint.startsWith("/api/auth/");

    if (
      response.status === 401 &&
      !isAuthEndpoint &&
      typeof window !== "undefined"
    ) {
      localStorage.removeItem("auth_token");
      window.location.href = "/login";
    }

    const body: ApiError = await response.json().catch(() => ({
      error: response.statusText,
      status: response.status,
    }));

    throw new Error(body.error);
  }

  if (response.status === 204) return undefined as T;
  const text = await response.text();
  if (!text) return undefined as T;
  return JSON.parse(text);
}

export const api = {
  get: <T>(endpoint: string) => request<T>(endpoint, { method: "GET" }),

  post: <T>(endpoint: string, data?: unknown) =>
    request<T>(endpoint, {
      method: "POST",
      body: data ? JSON.stringify(data) : undefined,
    }),

  put: <T>(endpoint: string, data: unknown) =>
    request<T>(endpoint, { method: "PUT", body: JSON.stringify(data) }),

  delete: <T>(endpoint: string) => request<T>(endpoint, { method: "DELETE" }),

  upload: async <T>(endpoint: string, file: File): Promise<T> => {
    const token = getToken();
    const formData = new FormData();
    formData.append("file", file);

    let response: Response;
    try {
      response = await fetch(`${BASE_URL}${endpoint}`, {
        method: "POST",
        headers: {
          ...(token ? { Authorization: `Bearer ${token}` } : {}),
        },
        body: formData,
      });
    } catch {
      throw new Error(
        "Cannot connect to the server. Make sure the backend is running.",
      );
    }

    if (!response.ok) {
      const body = await response.json().catch(() => ({
        error: response.statusText,
      }));
      throw new Error(body.error);
    }

    return response.json();
  },

  stream: async function* (
    endpoint: string,
    data: unknown,
  ): AsyncGenerator<string> {
    const token = getToken();

    let response: Response;
    try {
      response = await fetch(`${BASE_URL}${endpoint}`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          ...(token ? { Authorization: `Bearer ${token}` } : {}),
        },
        body: JSON.stringify(data),
      });
    } catch {
      throw new Error(
        "Cannot connect to the server. Make sure the backend is running.",
      );
    }

    if (!response.ok) {
      throw new Error(`Stream failed: ${response.statusText}`);
    }

    const reader = response.body?.getReader();
    if (!reader) throw new Error("No response body");

    const decoder = new TextDecoder();

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      const chunk = decoder.decode(value);
      const lines = chunk.split("\n");

      for (const line of lines) {
        if (line.startsWith("data: ")) {
          const data = line.slice(6);
          if (data === "[DONE]") return;
          yield data;
        }
      }
    }
  },
};
