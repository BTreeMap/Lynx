/**
 * Shape of an error surfaced by the shared apiClient (axios). The response body
 * follows the backend `ErrorResponse` contract (`{ error: string }`).
 */
interface ApiErrorShape {
    response?: { data?: { error?: string } };
}

/**
 * Extract a human-readable message from an unknown caught error, falling back
 * to the provided default when the API did not return a structured `error`.
 */
export const extractErrorMessage = (err: unknown, fallback: string): string =>
    (err as ApiErrorShape).response?.data?.error || fallback;
