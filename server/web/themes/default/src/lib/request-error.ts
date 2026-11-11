/** Keep transport details available without using them as the main message. */
export function requestErrorMessage(error: unknown): string {
  const detail = error instanceof Error ? error.message : String(error);
  const code = typeof error === "object" && error !== null && "code" in error ? error.code : undefined;
  if (code === 401) return "Your session has expired. Sign in again, then retry.";
  if (code === 403) return "Your account does not have permission for this request.";
  if (/unknown operation/i.test(detail)) return "This feature is not available on this server. Update the server to use it.";
  if (/timed out|timeout/i.test(detail)) return "The server took too long to respond. Please try again.";
  if (/csil: (connection|socket)|network|failed to fetch/i.test(detail)) {
    return "The server connection is unavailable. Check your connection, then retry.";
  }
  return "The request could not be completed. Please try again.";
}
