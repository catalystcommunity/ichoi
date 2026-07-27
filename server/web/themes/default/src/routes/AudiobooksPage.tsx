import { Show, type JSX } from "solid-js";
import { useI18n } from "../lib/i18n.tsx";
import { useServers } from "../stores/servers.tsx";
import { VirtualAlbumGrid } from "../components/VirtualAlbumGrid.tsx";
import { EmptyState } from "../components/common.tsx";

export function AudiobooksPage(): JSX.Element {
  const servers = useServers();
  const { t } = useI18n();
  return (
    <div class="page catalog-page">
      <header class="page-head">
        <div class="eyebrow">{t("nav.audiobooks")}</div>
        <h1 class="page-title">{t("audiobooks.title")}</h1>
        <p class="page-sub">{t("audiobooks.subtitle")}</p>
      </header>

      <div class="catalog-body">
        <Show when={servers.api()} fallback={<EmptyState title={t("errors.connectFirst")} />}>
          {(api) => (
            <div class="catalog-scroll">
              <VirtualAlbumGrid
                api={api()}
                library="audiobook"
                detailPath="audiobook"
              />
            </div>
          )}
        </Show>
      </div>
    </div>
  );
}
