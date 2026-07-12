import { createResource, createSignal, For, Show, type JSX } from "solid-js";
import { useI18n, LOCALES } from "../lib/i18n.tsx";
import { useServers } from "../stores/servers.tsx";
import { usePlayback } from "../stores/playback.tsx";
import { useTheme, type ThemeMode } from "../stores/theme.tsx";
import { CsilServiceError } from "../lib/csil.ts";
import type { TranscodeCodec } from "../lib/schema.ts";
import { EmptyState } from "../components/common.tsx";

export function SettingsPage(): JSX.Element {
  const { t, locale, setLocale } = useI18n();
  const theme = useTheme();
  const servers = useServers();
  const pb = usePlayback();

  const themeModes: { value: ThemeMode; label: string }[] = [
    { value: "system", label: t("settings.themeSystem") },
    { value: "light", label: t("settings.themeLight") },
    { value: "dark", label: t("settings.themeDark") },
  ];

  // Server settings (get-settings). Applies the server's theme default and lists
  // entries; editing requires admin (set-setting returns 403 otherwise).
  const [settings, { refetch }] = createResource(
    () => servers.api(),
    async (api) => {
      const s = await api.admin.getSettings();
      const themeDefault = s.entries["theme"];
      if (themeDefault === "light" || themeDefault === "dark" || themeDefault === "system") {
        theme.applyServerDefault(themeDefault);
      }
      return s;
    },
  );

  const isAdmin = () => servers.active()?.session?.role === "admin";
  const [saveError, setSaveError] = createSignal<string>();

  const saveSetting = async (key: string, value: string) => {
    const api = servers.api();
    if (!api) return;
    setSaveError(undefined);
    try {
      await api.admin.setSetting({ key, value });
      await refetch();
    } catch (err) {
      setSaveError(err instanceof CsilServiceError ? err.message : String(err));
    }
  };

  const pref = () => pb.pref();

  return (
    <div class="page">
      <header class="page-head">
        <div class="eyebrow">{t("nav.settings")}</div>
        <h1 class="page-title">{t("settings.title")}</h1>
      </header>

      <section class="panel" aria-labelledby="set-appearance">
        <h2 id="set-appearance">{t("settings.appearance")}</h2>
        <div class="field">
          <label id="theme-label">{t("settings.theme")}</label>
          <div class="segmented" role="radiogroup" aria-labelledby="theme-label">
            <For each={themeModes}>
              {(m) => (
                <button
                  role="radio"
                  aria-checked={theme.mode() === m.value}
                  aria-pressed={theme.mode() === m.value}
                  onClick={() => theme.setMode(m.value)}
                >
                  {m.label}
                </button>
              )}
            </For>
          </div>
        </div>
        <div class="field">
          <label for="locale">{t("settings.language")}</label>
          <select
            id="locale"
            class="select"
            value={locale()}
            onChange={(e) => setLocale(e.currentTarget.value)}
          >
            <For each={LOCALES}>{(l) => <option value={l}>{l}</option>}</For>
          </select>
        </div>
      </section>

      <section class="panel" aria-labelledby="set-playback">
        <h2 id="set-playback">{t("settings.playback")}</h2>
        <div class="field">
          <label for="transcode">{t("settings.transcode")}</label>
          <select
            id="transcode"
            class="select"
            value={pref().transcode_codec ?? "aac"}
            onChange={(e) =>
              pb.setPref({ ...pref(), transcode_codec: e.currentTarget.value as TranscodeCodec })
            }
          >
            <option value="aac">AAC-LC</option>
            <option value="mp3">MP3</option>
          </select>
        </div>
        <div class="field">
          <label class="row" style={{ gap: "10px", cursor: "pointer" }}>
            <input
              type="checkbox"
              checked={pref().prefer_original ?? false}
              onChange={(e) => pb.setPref({ ...pref(), prefer_original: e.currentTarget.checked })}
            />
            {t("settings.preferOriginal")}
          </label>
        </div>
        <div class="field">
          <label for="bitrate">{t("settings.maxBitrate")}</label>
          <input
            id="bitrate"
            class="input"
            type="number"
            min={0}
            value={pref().max_bitrate_kbps ?? 0}
            onInput={(e) =>
              pb.setPref({ ...pref(), max_bitrate_kbps: Number(e.currentTarget.value) || 0 })
            }
          />
        </div>
      </section>

      <section class="panel" aria-labelledby="set-server">
        <h2 id="set-server">{t("settings.server")}</h2>
        <p class="hint" style={{ "margin-bottom": "14px" }}>
          {t("settings.serverNote")}
        </p>
        <Show when={!isAdmin()}>
          <p class="hint" style={{ color: "var(--amber)" }}>
            {t("settings.adminOnly")}
          </p>
        </Show>
        <Show when={settings.error}>
          <p class="hint" style={{ color: "var(--text-faint)" }}>
            {t("errors.connectFirst")}
          </p>
        </Show>
        <Show
          when={!settings.error && settings() && Object.keys(settings()!.entries).length > 0}
          fallback={
            <Show when={!settings.error && settings.loading}>
              <EmptyState title={t("common.loading")} />
            </Show>
          }
        >
          <div style={{ display: "grid", gap: "10px" }}>
            <For each={Object.entries(settings()!.entries)}>
              {([key, value]) => (
                <div class="row spread" style={{ gap: "12px" }}>
                  <span class="mono chip">{key}</span>
                  <input
                    class="input"
                    style={{ "max-width": "260px" }}
                    value={value}
                    disabled={!isAdmin()}
                    onChange={(e) => void saveSetting(key, e.currentTarget.value)}
                  />
                </div>
              )}
            </For>
          </div>
        </Show>
        <Show when={saveError()}>
          <p class="hint" style={{ color: "var(--danger)", "margin-top": "10px" }}>
            {saveError()}
          </p>
        </Show>
      </section>
    </div>
  );
}
