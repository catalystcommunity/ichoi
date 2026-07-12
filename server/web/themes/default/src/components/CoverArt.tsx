// Album cover art. Fetches bytes via LibraryService.get-cover-art and shows them
// as an object URL, revoked on cleanup. Falls back to the album's initial in a
// tinted plate when there is no art (or no connection).
import { createResource, createSignal, onCleanup, Show, type JSX } from "solid-js";
import type { Album } from "../lib/schema.ts";
import { useServers } from "../stores/servers.tsx";

interface Props {
  album: Pick<Album, "id" | "title" | "has_cover_art">;
  maxSize?: number;
  class?: string;
}

export function CoverArt(props: Props): JSX.Element {
  const servers = useServers();
  const [url, setUrl] = createSignal<string>();

  const [data] = createResource(
    () => (props.album.has_cover_art ? { id: props.album.id, api: servers.api() } : null),
    async (input) => {
      if (!input?.api) return undefined;
      try {
        const art = await input.api.library.getCoverArt({
          album_id: input.id,
          max_size: props.maxSize ?? 512,
        });
        const blob = new Blob([art.data as BlobPart], { type: art.content_type });
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
