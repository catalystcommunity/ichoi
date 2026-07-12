// App root: providers + router. The router uses `Layout` as the root shell so the
// nav rail and transport persist across route changes.
import { lazy, type JSX } from "solid-js";
import { Router, Route } from "@solidjs/router";
import { I18nProvider } from "./lib/i18n.tsx";
import { ThemeProvider } from "./stores/theme.tsx";
import { ServersProvider } from "./stores/servers.tsx";
import { PlaybackProvider } from "./stores/playback.tsx";
import { Layout } from "./components/Layout.tsx";
import { LibraryPage } from "./routes/LibraryPage.tsx";

// Route-level code splitting keeps the initial bundle lean.
const AlbumPage = lazy(() => import("./routes/AlbumPage.tsx").then((m) => ({ default: m.AlbumPage })));
const ArtistPage = lazy(() => import("./routes/ArtistPage.tsx").then((m) => ({ default: m.ArtistPage })));
const SearchPage = lazy(() => import("./routes/SearchPage.tsx").then((m) => ({ default: m.SearchPage })));
const PlaylistsPage = lazy(() =>
  import("./routes/PlaylistsPage.tsx").then((m) => ({ default: m.PlaylistsPage })),
);
const JukeboxPage = lazy(() => import("./routes/JukeboxPage.tsx").then((m) => ({ default: m.JukeboxPage })));
const NowPlayingPage = lazy(() =>
  import("./routes/NowPlayingPage.tsx").then((m) => ({ default: m.NowPlayingPage })),
);
const SettingsPage = lazy(() => import("./routes/SettingsPage.tsx").then((m) => ({ default: m.SettingsPage })));

export function App(): JSX.Element {
  return (
    <I18nProvider>
      <ThemeProvider>
        <ServersProvider>
          <PlaybackProvider>
            <Router root={Layout}>
              <Route path="/" component={LibraryPage} />
              <Route path="/album/:id" component={AlbumPage} />
              <Route path="/artist/:id" component={ArtistPage} />
              <Route path="/search" component={SearchPage} />
              <Route path="/playlists" component={PlaylistsPage} />
              <Route path="/jukebox" component={JukeboxPage} />
              <Route path="/now-playing" component={NowPlayingPage} />
              <Route path="/settings" component={SettingsPage} />
              <Route path="*" component={LibraryPage} />
            </Router>
          </PlaybackProvider>
        </ServersProvider>
      </ThemeProvider>
    </I18nProvider>
  );
}
