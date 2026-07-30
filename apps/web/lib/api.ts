export class ApiFailure extends Error {
  constructor(
    public readonly status: number,
    message: string,
  ) {
    super(message)
  }
}

export async function api<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers)
  if (init.body && typeof init.body === 'string') headers.set('content-type', 'application/json')
  const response = await fetch(path, { ...init, headers, credentials: 'include' })
  if (!response.ok) {
    const body = (await response.json().catch(() => undefined)) as
      { message?: string; error?: { message?: string } } | undefined
    throw new ApiFailure(
      response.status,
      body?.message ?? body?.error?.message ?? 'Luna could not complete the request.',
    )
  }
  if (response.status === 204) return undefined as T
  const text = await response.text()
  return (text ? JSON.parse(text) : undefined) as T
}

export function messageFromError(error: unknown): string {
  return error instanceof Error ? error.message : 'Luna could not complete the request.'
}
