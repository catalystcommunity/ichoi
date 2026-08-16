export interface OutputTarget {
  id: string;
  name: string;
}

export interface OwnedTargetStore {
  servers: Record<string, string[]>;
  legacy: string[];
}

export function isBrowserShareTarget(id: string): boolean {
  return id.startsWith("share:");
}

export function parseOwnedTargetStore(raw: string | null): OwnedTargetStore {
  if (!raw) return { servers: {}, legacy: [] };
  try {
    const parsed: unknown = JSON.parse(raw);
    if (Array.isArray(parsed)) {
      return {
        servers: {},
        legacy: parsed.filter(
          (id): id is string => typeof id === "string" && isBrowserShareTarget(id),
        ),
      };
    }
    if (!parsed || typeof parsed !== "object") return { servers: {}, legacy: [] };
    const candidate = (parsed as { servers?: unknown }).servers;
    if (!candidate || typeof candidate !== "object" || Array.isArray(candidate)) {
      return { servers: {}, legacy: [] };
    }
    const servers: Record<string, string[]> = {};
    for (const [serverId, ids] of Object.entries(candidate)) {
      if (Array.isArray(ids)) {
        servers[serverId] = ids.filter(
          (id): id is string => typeof id === "string" && isBrowserShareTarget(id),
        );
      }
    }
    return { servers, legacy: [] };
  } catch {
    return { servers: {}, legacy: [] };
  }
}

export function firstOwnedTarget(
  targets: OutputTarget[],
  ownedIds: string[],
): OutputTarget | undefined {
  return targets.find((target) => ownedIds.includes(target.id));
}

export function resolveOutputTarget(
  requestedId: string,
  targets: OutputTarget[],
  ownedIds: string[],
): string {
  if (requestedId !== "local") return requestedId;
  return firstOwnedTarget(targets, ownedIds)?.id ?? requestedId;
}

export function outputTargetName(name: string, mine: boolean, mineLabel: string): string {
  return mine ? `${name} (${mineLabel})` : name;
}
