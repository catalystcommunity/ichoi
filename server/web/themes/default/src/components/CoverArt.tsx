// Album cover art. Fetches bytes via LibraryService.get-cover-art and shows them
// as an object URL, revoked on cleanup. Falls back to the album's initial in a
// tinted plate when there is no art (or no connection).
import { createResource, createSignal, onCleanup, Show, type JSX } from "solid-js";
import type { Album } from "../lib/schema.ts";
import { cachedCover } from "../lib/cover-art-cache.ts";
import { useServers } from "../stores/servers.tsx";

interface Props {
  album: Pick<Album, "id" | "title" | "has_cover_art">;
  maxSize?: number;
  class?: string;
  serverId?: string;
}

export function CoverArt(props: Props): JSX.Element {
  const servers = useServers();
  const [url, setUrl] = createSignal<string>();
  let disposed = false;

  const [data] = createResource(
    () => {
      if (!props.album.has_cover_art) return null;
      if (props.serverId) servers.servers.find((server) => server.id === props.serverId)?.state;
      return {
        id: props.album.id,
        maxSize: props.maxSize ?? 512,
        api: props.serverId ? servers.apiFor(props.serverId) : servers.api(),
      };
    },
    async (input) => {
      if (!input?.api) return undefined;
      try {
        const blob = await cachedCover(input.api, input.id, input.maxSize);
        if (disposed) return undefined;
        const objUrl = URL.createObjectURL(blob);
        setUrl((prev) => {
          if (prev) URL.revokeObjectURL(prev);
          return objUrl;
        });
        return objUrl;
      } catch {
        return undefined;
      }
    },
  );

  onCleanup(() => {
    disposed = true;
    const u = url();
    if (u) URL.revokeObjectURL(u);
  });

  const initial = () => props.album.title.trim().charAt(0).toUpperCase() || "♪";

  return (
    <div class={`cover ${props.class ?? ""}`}>
      <Show when={data()} fallback={<span class="cover-fallback">{initial()}</span>}>
        <img src={url()} alt="" loading="lazy" />
      </Show>
    </div>
  );
}
