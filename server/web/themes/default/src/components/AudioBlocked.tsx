// Two views of one fact: a browser satellite is connected but cannot make sound until
// somebody uses it (§6.4). `SatelliteAudioBlocked` tells the person standing at the
// satellite; `TargetAudioBlocked` tells a controller who just sent music to it.
import { createEffect, createSignal, type JSX } from "solid-js";
import { useI18n } from "../lib/i18n.tsx";
import { usePlayback } from "../stores/playback.tsx";
import { Dialog } from "./Dialog.tsx";

/** Shown on the satellite itself while its audio is blocked.
 *
 * The dialog does not do the unlocking — any gesture on the page does that (see
 * `onFirstGesture`), including the one that dismisses this dialog. It exists so a person who
 * walks up to a silent screen learns what to do instead of finding nothing. */
export function SatelliteAudioBlocked(): JSX.Element {
  const pb = usePlayback();
  const { t } = useI18n();
  const [dismissed, setDismissed] = createSignal(false);

  // Offer it again if audio becomes blocked a second time (a reconnect, a failed play).
  createEffect(() => {
    if (!pb.audioBlocked()) setDismissed(false);
  });

  return (
    <Dialog
      open={pb.audioBlocked() && !dismissed()}
      title={t("player.audioBlockedTitle")}
      onClose={() => setDismissed(true)}
    >
      <p class="hint">{t("player.audioBlockedBody")}</p>
      <div class="dialog-actions">
        <button type="button" class="btn btn-ghost" onClick={() => setDismissed(true)}>
          {t("player.audioBlockedDismiss")}
        </button>
        <button
          type="button"
          class="btn btn-primary"
          onClick={() =>
            void pb.enableOutputAudio().catch((error) => console.warn("[audio] unlock", error))
          }
        >
          {t("player.enableAudio")}
        </button>
      </div>
    </Dialog>
  );
}

/** Shown to a controller who selected an output that cannot play yet. `name` is the output;
 * `onClose` clears the caller's "which target did they pick" state. */
export function TargetAudioBlocked(props: {
  name?: string;
  onClose: () => void;
}): JSX.Element {
  const { t } = useI18n();
  return (
    <Dialog
      open={props.name !== undefined}
      title={t("jukebox.targetBlockedTitle", { name: props.name ?? "" })}
      onClose={props.onClose}
    >
      <p class="hint">{t("jukebox.targetBlockedBody")}</p>
      <div class="dialog-actions">
        <button type="button" class="btn btn-primary" onClick={props.onClose}>
          {t("common.ok")}
        </button>
      </div>
    </Dialog>
  );
}
