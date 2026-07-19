import { createResource, For, Show, type JSX } from "solid-js";
import { useI18n } from "../lib/i18n.tsx";
import { useServers } from "../stores/servers.tsx";
import { AlbumTile } from "../components/AlbumTile.tsx";
import { EmptyState, Spinner } from "../components/common.tsx";

export function AudiobooksPage(): JSX.Element {
  const servers = useServers();
  const { t } = useI18n();
  const [books] = createResource(
    () => servers.api(),
    (api) => api!.library.listAlbums({ library: "audiobook", limit: 500 }),
  );

  return (
    <div class="page">
      <header class="page-head">
        <div class="eyebrow">{t("nav.audiobooks")}</div>
        <h1 class="page-title">{t("audiobooks.title")}</h1>
        <p class="page-sub">{t("audiobooks.subtitle")}</p>
      </header>

      <Show when={servers.api()} fallback={<EmptyState title={t("errors.connectFirst")} />}>
        <Show when={!books.loading} fallback={<Spinner label={t("audiobooks.loading")} />}>
          <Show
            when={(books()?.albums.length ?? 0) > 0}
            fallback={<EmptyState title={t("audiobooks.empty")} />}
          >
            <div class="grid">
              <For each={books()!.albums}>
                {(book) => (
                  <AlbumTile
                    album={book}
                    href={`/audiobook/${encodeURIComponent(book.id)}`}
                  />
                )}
              </For>
            </div>
          </Show>
        </Show>
      </Show>
    </div>
  );
}
