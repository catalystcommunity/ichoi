// The Jukebox (DESIGN §6): shared targets rendered as glowing console channel
// strips with live transport, plus this client's private players and a control to
// share this device (§6.4). Shared-target state streams over PlayerService.subscribe.
import {
  createResource,
  createSignal,
  For,
  onCleanup,
  Show,
  type JSX,
} from "solid-js";
import { useI18n } from "../lib/i18n.tsx";
import { useServers } from "../stores/servers.tsx";
import { usePlayback } from "../stores/playback.tsx";
import type { Player, PlayerCommand, PlayerState } from "../lib/schema.ts";
import { CsilServiceError } from "../lib/csil.ts";
import { EmptyState, Spinner } from "../components/common.tsx";
import { Dialog } from "../components/Dialog.tsx";
import { formatDuration } from "../lib/format.ts";
import {
  IconBroadcast,
  IconNext,
  IconPause,
  IconPlay,
  IconPrev,
} from "../components/Icons.tsx";

export function JukeboxPage(): JSX.Element {
  const servers = useServers();
  const pb = usePlayback();
  const { t } = useI18n();
  const [showShare, setShowShare] = createSignal(false);
  const [suffix, setSuffix] = createSignal("Device");
  const [shareError, setShareError] = createSignal<string>();

  const [players, { refetch }] = createResource(
    () => servers.api(),
    (api) => api.player.listPlayers(),
  );

  const shared = () => players()?.players.filter((p) => p.kind === "shared") ?? [];
  const priv = () => players()?.players.filter((p) => p.kind === "private") ?? [];

  const enableShare = async (e: Event) => {
    e.preventDefault();
    setShareError(undefined);
    const api = servers.api();
    if (!api) return;
    try {
      const res = await api.player.enableShare({ suffix: suffix().trim() || undefined });
      // This client owns the device it just shared: remember it (so it auto-outputs on
      // reload) and make it the active target so playback here drives its server queue.
      pb.markOwned(res.player.id);
      pb.setTarget(res.player.id);
      setShowShare(false);
      await refetch();
    } catch (err) {
      setShareError(err instanceof CsilServiceError ? err.message : String(err));
    }
  };

  return (
    <div class="page">
      <header class="page-head">
        <div class="eyebrow">{t("nav.jukebox")}</div>
        <div class="row spread">
          <div>
            <h1 class="page-title">{t("jukebox.title")}</h1>
            <p class="page-sub">{t("jukebox.subtitle")}</p>
          </div>
          <Show
            when={pb.owned().length > 0}
            fallback={
              <button type="button" class="btn" onClick={() => setShowShare(true)}>
                <IconBroadcast size={16} /> {t("jukebox.shareThisDevice")}
              </button>
            }
          >
            <button
              type="button"
              class="btn"
              onClick={() => {
                for (const id of pb.owned()) void pb.releaseDevice(id);
              }}
            >
              <IconBroadcast size={16} /> {t("jukebox.stopSharingThisDevice")}
            </button>
          </Show>
        </div>
      </header>

      <Show when={servers.api()} fallback={<EmptyState title={t("errors.connectFirst")} />}>
        <Show when={!players.loading} fallback={<Spinner label={t("common.loading")} />}>
          <div class="section-head">
            <h2>{t("jukebox.shared")}</h2>
            <span class="count">{shared().length}</span>
          </div>
          <Show
            when={shared().length > 0}
            fallback={<EmptyState title={t("jukebox.empty")} />}
          >
            <div class="strip-grid">
              <For each={shared()}>{(player) => <ChannelStrip player={player} />}</For>
            </div>
          </Show>

          <Show when={priv().length > 0}>
            <div class="section-head">
              <h2>{t("jukebox.private")}</h2>
              <span class="count">{priv().length}</span>
            </div>
            <div class="strip-grid">
              <For each={priv()}>
                {(player) => (
                  <div class="strip">
                    <div class="strip-top">
                      <div>
                        <div class="strip-name">{player.name}</div>
                        <div class="strip-where">{t("jukebox.private")}</div>
                      </div>
                    </div>
                  </div>
                )}
              </For>
            </div>
          </Show>
        </Show>
      </Show>

      <Dialog open={showShare()} title={t("jukebox.shareTitle")} onClose={() => setShowShare(false)}>
        <form onSubmit={enableShare}>
          <div class="field">
            <label for="share-suffix">{t("jukebox.suffix")}</label>
            <input
              id="share-suffix"
              class="input"
              value={suffix()}
              onInput={(e) => setSuffix(e.currentTarget.value)}
              maxLength={48}
            />
            <span class="hint">{t("jukebox.suffixHint")}</span>
          </div>
          <Show when={shareError()}>
            <p class="hint" style={{ color: "var(--danger)" }}>
              {shareError()}
            </p>
          </Show>
          <div class="dialog-actions">
            <button type="button" class="btn btn-ghost" onClick={() => setShowShare(false)}>
              {t("servers.cancel")}
            </button>
            <button type="submit" class="btn btn-primary">
              {t("jukebox.enableShare")}
            </button>
          </div>
        </form>
      </Dialog>
    </div>
  );
}

/** One shared target as a console channel strip: live now-playing, transport, and
 * a volume fader. Subscribes to the target's `PlayerState` stream for the life of
 * the component. */
function ChannelStrip(props: { player: Player }): JSX.Element {
  const servers = useServers();
  const pb = usePlayback();
  const { t } = useI18n();
  const [state, setState] = createSignal<PlayerState>();

  const isOutput = () => pb.owned().includes(props.player.id);

  const api = servers.api();
  if (api) {
    const off = api.player.subscribe({ player_id: props.player.id }, (s) => {
      // The channel fans every player's state; keep only this strip's.
      if (s.player_id === props.player.id) setState(s);
    });
    onCleanup(off);
  }

  const status = () => state()?.status ?? "stopped";
  const onAir = () => status() === "playing";
  const nowTrack = () => {
    const s = state();
    if (!s || s.current_index === undefined) return undefined;
    return s.queue[s.current_index];
  };

  const send = async (command: PlayerCommand) => {
    const a = servers.api();
    if (!a) return;
    try {
      const next = await a.player.control({ player_id: props.player.id, command });
      setState(next);
    } catch (err) {
      console.error("[jukebox] control failed", err);
    }
  };

  return (
    <section class={`strip ${onAir() ? "on-air" : ""}`} aria-label={props.player.name}>
      <div class="strip-top">
        <div>
          <div class="strip-name">{props.player.name}</div>
          <div class="strip-where">{t("jukebox.shared")}</div>
        </div>
        <span class="on-air-lamp">
          <span class="lamp" aria-hidden="true" />
          {onAir() ? t("jukebox.onAir") : t("jukebox.idle")}
        </span>
      </div>

      <div class="strip-now">
        <Show
          when={nowTrack()}
          fallback={<div class="strip-artist">{t("jukebox.noQueue")}</div>}
        >
          <div class="strip-track">{nowTrack()!.title ?? nowTrack()!.track_id}</div>
          <Show when={nowTrack()!.artist}>
            <div class="strip-artist">{nowTrack()!.artist}</div>
          </Show>
          <Show when={state()?.position_ms !== undefined}>
            <div class="chip mono">
              {formatDuration(state()!.position_ms)} /{" "}
              {formatDuration(nowTrack()!.duration_ms ?? 0)}
            </div>
          </Show>
        </Show>
      </div>

      <div class="transport-row">
        <button
          type="button"
          class="btn btn-ghost btn-icon"
          aria-label={t("jukebox.previous")}
          onClick={() => void send({ op: "previous" })}
        >
          <IconPrev size={18} />
        </button>
        <button
          type="button"
          class="btn btn-ghost btn-icon"
          aria-label={onAir() ? t("jukebox.pause") : t("jukebox.play")}
          onClick={() => void send(onAir() ? { op: "pause" } : { op: "play" })}
        >
          <Show when={onAir()} fallback={<IconPlay size={18} />}>
            <IconPause size={18} />
          </Show>
        </button>
        <button
          type="button"
          class="btn btn-ghost btn-icon"
          aria-label={t("jukebox.next")}
          onClick={() => void send({ op: "next" })}
        >
          <IconNext size={18} />
        </button>
      </div>

      <label class="row" style={{ gap: "10px" }}>
        <span class="eyebrow">{t("jukebox.volume")}</span>
        <input
          class="slider"
          type="range"
          min={0}
          max={100}
          value={state()?.volume ?? 100}
          aria-label={`${props.player.name} ${t("jukebox.volume")}`}
          onChange={(e) => void send({ op: "volume", volume: Number(e.currentTarget.value) })}
        />
      </label>

      <div class="strip-footer row spread">
        <Show
          when={isOutput()}
          fallback={
            <button type="button" class="btn btn-ghost" onClick={() => void pb.claimOutput(props.player.id)}>
              {t("jukebox.playHere")}
            </button>
          }
        >
          <span class="chip">{t("jukebox.outputHere")}</span>
          <button type="button" class="btn btn-ghost" onClick={() => void pb.releaseDevice(props.player.id)}>
            {t("jukebox.stopSharing")}
          </button>
        </Show>
      </div>
    </section>
  );
}
