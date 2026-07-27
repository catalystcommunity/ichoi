// The app shell: nav rail (brand + primary nav + server switcher) on the left,
// routed content in the middle, the persistent transport pinned to the bottom.
import {
  createEffect,
  createResource,
  createSignal,
  ErrorBoundary,
  For,
  onCleanup,
  Show,
  type JSX,
} from "solid-js";
import { A, useLocation, useNavigate } from "@solidjs/router";
import { useI18n } from "../lib/i18n.tsx";
import { useServers } from "../stores/servers.tsx";
import { EmptyState } from "./common.tsx";
import { AuthArea, ServerSwitcher } from "./ServerSwitcher.tsx";
import { Transport } from "./Transport.tsx";
import { VersionFooter } from "./VersionFooter.tsx";
import {
  IconJukebox,
  IconBook,
  IconBroadcast,
  IconLibrary,
  IconNowPlaying,
  IconPlaylist,
  IconSearch,
  IconSettings,
} from "./Icons.tsx";
import { satelliteToken } from "../lib/satellite-mode.ts";

const NAV = [
  { href: "/", key: "nav.library", icon: IconLibrary, end: true },
  { href: "/search", key: "nav.search", icon: IconSearch },
  { href: "/playlists", key: "nav.playlists", icon: IconPlaylist },
  { href: "/jukebox", key: "nav.jukebox", icon: IconJukebox },
  { href: "/now-playing", key: "nav.nowPlaying", icon: IconNowPlaying },
  { href: "/settings", key: "nav.settings", icon: IconSettings },
] as const;

const SATELLITE_NAV = [
  ...NAV.filter((item) => item.href !== "/jukebox" && item.href !== "/settings"),
  { href: "/satellite", key: "nav.satellite", icon: IconBroadcast },
] as const;

export function Layout(props: { children?: JSX.Element }): JSX.Element {
  const { t } = useI18n();
  const servers = useServers();
  const location = useLocation();
  const navigate = useNavigate();
  const satelliteMode = Boolean(satelliteToken());
  const [updating, setUpdating] = createSignal(false);
  const [access] = createResource(async () => {
    const response = await fetch("/api/auth", { cache: "no-store" });
    if (!response.ok) return { guest_allowed: true };
    return await response.json() as { guest_allowed?: boolean };
  });
  const signInRequired = () =>
    !satelliteMode &&
    access()?.guest_allowed === false &&
    (!servers.active()?.session ||
      servers.active()?.session?.account_id === "__guest__");
  const updateListener = () => setUpdating(true);
  window.addEventListener("ichoi:update-reloading", updateListener);
  onCleanup(() => window.removeEventListener("ichoi:update-reloading", updateListener));
  let initialSatelliteRoute = true;
  createEffect(() => {
    const path = location.pathname;
    if (satelliteMode && initialSatelliteRoute) {
      initialSatelliteRoute = false;
      if (path === "/") {
        navigate("/now-playing", { replace: true });
        return;
      }
    }
    if (
      satelliteMode &&
      (path === "/jukebox" || path.startsWith("/settings"))
    ) {
      navigate("/now-playing", { replace: true });
    }
  });
  const [libraries] = createResource(
    () => servers.api(),
    (api) => api!.library.listLibraries(),
  );
  const hasAudiobooks = () =>
    libraries()?.libraries.some((library) => library.kind === "audiobook") ?? false;
  return (
    <Show when={location.pathname !== "/satellite"} fallback={props.children}>
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
          <For each={satelliteMode ? SATELLITE_NAV : NAV}>
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
        <Show
          when={!satelliteMode}
          fallback={<span class="chip">{servers.active()?.session?.handle ?? "Satellite"}</span>}
        >
          <ServerSwitcher />
        </Show>
      </nav>

      <main class="main" id="main-content" tabindex="-1">
        <div class="main-route">
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
            <Show
              when={!signInRequired()}
              fallback={
                <div class="page auth-required">
                  <EmptyState title={t("auth.required")} hint={t("auth.requiredHint")}>
                    <div style={{ "max-width": "320px", margin: "18px auto 0" }}>
                      <AuthArea />
                    </div>
                  </EmptyState>
                </div>
              }
            >
              {props.children}
            </Show>
          </ErrorBoundary>
        </div>
        <VersionFooter />
      </main>

      <Transport />
      <Show when={updating()}>
        <div class="update-reload" role="status" aria-live="assertive">
          Updating Ichoi and restoring this satellite…
        </div>
      </Show>
    </div>
    </Show>
  );
}
