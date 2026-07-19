// The app shell: nav rail (brand + primary nav + server switcher) on the left,
// routed content in the middle, the persistent transport pinned to the bottom.
import { createResource, ErrorBoundary, For, Show, type JSX } from "solid-js";
import { A } from "@solidjs/router";
import { useI18n } from "../lib/i18n.tsx";
import { useServers } from "../stores/servers.tsx";
import { EmptyState } from "./common.tsx";
import { ServerSwitcher } from "./ServerSwitcher.tsx";
import { Transport } from "./Transport.tsx";
import {
  IconJukebox,
  IconBook,
  IconLibrary,
  IconNowPlaying,
  IconPlaylist,
  IconSearch,
  IconSettings,
} from "./Icons.tsx";

const NAV = [
  { href: "/", key: "nav.library", icon: IconLibrary, end: true },
  { href: "/search", key: "nav.search", icon: IconSearch },
  { href: "/playlists", key: "nav.playlists", icon: IconPlaylist },
  { href: "/jukebox", key: "nav.jukebox", icon: IconJukebox },
  { href: "/now-playing", key: "nav.nowPlaying", icon: IconNowPlaying },
  { href: "/settings", key: "nav.settings", icon: IconSettings },
] as const;

export function Layout(props: { children?: JSX.Element }): JSX.Element {
  const { t } = useI18n();
  const servers = useServers();
  const [libraries] = createResource(
    () => servers.api(),
    (api) => api!.library.listLibraries(),
  );
  const hasAudiobooks = () =>
    libraries()?.libraries.some((library) => library.kind === "audiobook") ?? false;
  return (
    <div class="app-shell">
      <a class="skip-link" href="#main-content">
        {t("nav.skipToContent")}
      </a>

      <nav class="rail" aria-label="Primary">
        <div class="brand">
          <span class="brand-mark" aria-hidden="true" />
          <span>
            <span class="brand-name">{t("app.name")}</span>
            <br />
            <span class="brand-tag">{t("app.tagline")}</span>
          </span>
        </div>

        <ul class="nav" role="list">
          <For each={NAV}>
            {(item) => (
              <>
                <li>
                  <A
                    href={item.href}
                    end={"end" in item ? item.end : false}
                    class="nav-link"
                    activeClass="active"
                  >
                    <item.icon />
                    <span>{t(item.key)}</span>
                  </A>
                </li>
                <Show when={item.href === "/" && hasAudiobooks()}>
                  <li>
                    <A href="/audiobooks" class="nav-link" activeClass="active">
                      <IconBook />
                      <span>{t("nav.audiobooks")}</span>
                    </A>
                  </li>
                </Show>
              </>
            )}
          </For>
        </ul>

        <div class="rail-spacer" />
        <ServerSwitcher />
      </nav>

      <main class="main" id="main-content" tabindex="-1">
        <ErrorBoundary
          fallback={(err, reset) => (
            <div class="page">
              <EmptyState title={t("errors.generic")} hint={String(err?.message ?? err)}>
                <button
                  type="button"
                  class="btn btn-primary"
                  onClick={() => {
                    const id = servers.activeId();
                    if (id) void servers.reconnect(id);
                    reset();
                  }}
                >
                  {t("errors.retry")}
                </button>
              </EmptyState>
            </div>
          )}
        >
          {props.children}
        </ErrorBoundary>
      </main>

      <Transport />
    </div>
  );
}
