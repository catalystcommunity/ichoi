export const SATELLITE_TOKEN_KEY = "ichoi.satelliteToken";
export const SATELLITE_OUTPUT_KEY = "ichoi.satelliteOutput";

export function satelliteToken(): string | undefined {
  try {
    return localStorage.getItem(SATELLITE_TOKEN_KEY) || undefined;
  } catch {
    return undefined;
  }
}

export function enterSatelliteMode(token: string): void {
  localStorage.setItem(SATELLITE_TOKEN_KEY, token);
  // Mode changes are explicit credential changes. Do not silently retain a normal
  // user session that would spring back into use when satellite mode is left.
  try {
    const servers = JSON.parse(localStorage.getItem("ichoi.servers") ?? "[]") as Array<Record<string, unknown>>;
    for (const server of servers) delete server.token;
    localStorage.setItem("ichoi.servers", JSON.stringify(servers));
  } catch {
    /* a malformed ordinary-server cache should not prevent satellite setup */
  }
}

export function leaveSatelliteMode(): void {
  localStorage.removeItem(SATELLITE_TOKEN_KEY);
}
