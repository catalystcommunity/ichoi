import { createResource, Show, type JSX } from "solid-js";

interface StatusResponse {
  version?: string;
}

export function VersionFooter(): JSX.Element {
  const [status] = createResource(async () => {
    const response = await fetch("/status", { cache: "no-store" });
    if (!response.ok) throw new Error(`status request failed: ${response.status}`);
    return await response.json() as StatusResponse;
  });

  return (
    <footer class="version-footer">
      <a href="https://catalystichoi.com">Powered by Ichoi</a>
      <Show when={status()?.version}>
        {(version) => <span aria-label={`Ichoi version ${version()}`}> · v{version()}</span>}
      </Show>
    </footer>
  );
}
