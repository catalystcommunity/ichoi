import { createResource, For, Show, type JSX } from "solid-js";
import { useNavigate, useParams } from "@solidjs/router";
import { useI18n } from "../lib/i18n.tsx";
import { useServers } from "../stores/servers.tsx";
import { AlbumTile } from "../components/AlbumTile.tsx";
import { EmptyState, Spinner } from "../components/common.tsx";
import { IconChevronLeft } from "../components/Icons.tsx";

export function ArtistPage(): JSX.Element {
  const params = useParams();
  const navigate = useNavigate();
  const servers = useServers();
  const { t } = useI18n();

  const [detail] = createResource(
    () => {
      const api = servers.api();
      return api && params.id ? { api, id: params.id } : undefined;
    },
    (input) => input.api.library.getArtist({ artist_id: input.id }),
  );

  return (
    <div class="page">
      <button type="button" class="btn btn-ghost" onClick={() => navigate("/")}>
        <IconChevronLeft size={16} /> {t("library.artists")}
      </button>

      <Show when={!detail.loading} fallback={<div style={{ "margin-top": "30px" }}><Spinner /></div>}>
        <Show when={detail()} fallback={<EmptyState title={t("errors.generic")} />}>
          {(d) => (
            <>
              <header class="page-head" style={{ "margin-top": "18px" }}>
                <div class="eyebrow">{t("library.artists")}</div>
                <h1 class="page-title">{d().artist.name}</h1>
                <p class="page-sub">{t("library.albumsCount", { count: d().artist.album_count })}</p>
              </header>
              <Show
                when={d().albums.length > 0}
                fallback={<EmptyState title={t("library.noAlbums")} />}
              >
                <div class="grid">
                  <For each={d().albums}>{(album) => <AlbumTile album={album} />}</For>
                </div>
              </Show>
            </>
          )}
        </Show>
      </Show>
    </div>
  );
}
