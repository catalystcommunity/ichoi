// A minimal accessible modal dialog: role="dialog", aria-modal, labelled by its
// heading, Escape to close, backdrop click to close, and focus moved inside on
// open / restored on close. Enough for this app's small forms.
import { createEffect, onCleanup, type JSX } from "solid-js";
import { Portal } from "solid-js/web";

interface Props {
  open: boolean;
  title: string;
  onClose: () => void;
  children: JSX.Element;
}

export function Dialog(props: Props): JSX.Element {
  let panel: HTMLDivElement | undefined;
  let previouslyFocused: HTMLElement | null = null;

  createEffect(() => {
    if (props.open) {
      previouslyFocused = document.activeElement as HTMLElement | null;
      queueMicrotask(() => {
        const focusable = panel?.querySelector<HTMLElement>(
          'input, button, select, textarea, [tabindex]:not([tabindex="-1"])',
        );
        focusable?.focus();
      });
    } else {
      previouslyFocused?.focus?.();
    }
  });

  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") props.onClose();
  };

  createEffect(() => {
    if (props.open) {
      document.addEventListener("keydown", onKey);
      onCleanup(() => document.removeEventListener("keydown", onKey));
    }
  });

  return (
    <Portal>
      {props.open && (
        <div
          class="dialog-backdrop"
          onClick={(e) => {
            if (e.target === e.currentTarget) props.onClose();
          }}
        >
          <div
            ref={panel}
            class="dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="dialog-title"
          >
            <h2 id="dialog-title">{props.title}</h2>
            {props.children}
          </div>
        </div>
      )}
    </Portal>
  );
}
