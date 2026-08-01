// Browser autoplay policy helpers for the satellite PWA (§6.4).
//
// A satellite is driven from the server: the queue changes somewhere else and this page is
// told to play. A browser refuses that until the page has user activation, so a satellite
// that nobody has touched since its last load is reachable, controllable, and silent.
//
// These helpers answer three questions the satellite needs:
//   - Is this an installed PWA?     (installed apps are exempt from the autoplay block)
//   - Can this page play sound now? (probeAutoplay)
//   - Tell me when a person touches the page, once. (onFirstGesture)

/** True when the page runs as an installed app rather than a browser tab. Chrome exempts
 * installed PWAs from the autoplay block, so a satellite installed on a Chromebook never
 * needs the manual unlock. */
export function isInstalledPwa(): boolean {
  try {
    if (typeof matchMedia === "function") {
      // "standalone" covers the ordinary install; a Chromebook launcher shortcut can also
      // report "minimal-ui", "fullscreen", or "window-controls-overlay".
      for (const mode of ["standalone", "minimal-ui", "fullscreen", "window-controls-overlay"]) {
        if (matchMedia(`(display-mode: ${mode})`).matches) return true;
      }
    }
  } catch {
    /* a browser without matchMedia is a tab */
  }
  // iOS Safari predates display-mode and reports installation here instead.
  return (navigator as Navigator & { standalone?: boolean }).standalone === true;
}

interface AutoplayNavigator extends Navigator {
  getAutoplayPolicy?: (type: "mediaelement" | "audiocontext") => string;
}

/** Whether this page can start audio on its own right now.
 *
 * Chrome 120+ answers directly through `navigator.getAutoplayPolicy`, which is the accurate
 * source and needs no side effects. Elsewhere we infer it from a scratch `AudioContext`: a
 * page without user activation gets one that is born "suspended". */
export async function probeAutoplay(): Promise<boolean> {
  const policy = (navigator as AutoplayNavigator).getAutoplayPolicy;
  if (typeof policy === "function") {
    try {
      // "allowed-muted" means sound is still blocked, so only "allowed" passes.
      return policy.call(navigator, "mediaelement") === "allowed";
    } catch {
      /* fall through to the AudioContext probe */
    }
  }
  const AudioContextClass = globalThis.AudioContext;
  if (!AudioContextClass) return true; // No way to tell; do not cry wolf.
  let context: AudioContext | undefined;
  try {
    context = new AudioContextClass();
    return context.state === "running";
  } catch {
    return false;
  } finally {
    // Contexts are a limited per-page resource, so never leak the probe.
    await context?.close().catch(() => undefined);
  }
}

const GESTURE_EVENTS = ["pointerdown", "touchend", "keydown", "click"] as const;

/** Run `handler` on the first user gesture anywhere in the page, then stop listening.
 *
 * Any gesture grants user activation, so the unlock does not need its own button: the first
 * tap on the queue, a keypress, or a click on the dialog all satisfy the policy. Listeners
 * are passive and in the capture phase so nothing that stops propagation can hide the
 * gesture from us, and so we never delay the interaction the person actually wanted.
 *
 * Returns a function that removes the listeners early. */
export function onFirstGesture(handler: () => void): () => void {
  let done = false;
  const fire = () => {
    if (done) return;
    done = true;
    remove();
    handler();
  };
  const remove = () => {
    for (const name of GESTURE_EVENTS) {
      document.removeEventListener(name, fire, true);
    }
  };
  for (const name of GESTURE_EVENTS) {
    document.addEventListener(name, fire, { capture: true, passive: true });
  }
  return remove;
}
