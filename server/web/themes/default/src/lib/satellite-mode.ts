export const SATELLITE_TOKEN_KEY = "ichoi.satelliteToken";
export const SATELLITE_OUTPUT_KEY = "ichoi.satelliteOutput";
export const SATELLITE_OUTPUT_NAME_KEY = "ichoi.satelliteOutputName";

export function satelliteToken(): string | undefined {
  try {
    return localStorage.getItem(SATELLITE_TOKEN_KEY) || undefined;
  } catch {
    return undefined;
  }
}

export function satelliteOutput(): { id: string; name: string } {
  try {
    return {
      id: localStorage.getItem(SATELLITE_OUTPUT_KEY) || "default",
      name: localStorage.getItem(SATELLITE_OUTPUT_NAME_KEY) || "Default audio output",
    };
  } catch {
    return { id: "default", name: "Default audio output" };
  }
}

export function setSatelliteOutput(id: string, name: string): void {
  localStorage.setItem(SATELLITE_OUTPUT_KEY, id);
  localStorage.setItem(SATELLITE_OUTPUT_NAME_KEY, name);
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
