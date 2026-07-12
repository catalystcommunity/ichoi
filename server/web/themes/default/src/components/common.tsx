// Small shared presentational pieces.
import { For, Show, type JSX } from "solid-js";

export function Spinner(props: { label?: string }): JSX.Element {
  return (
    <span class="row" role="status" aria-live="polite">
      <span class="spinner" aria-hidden="true" />
      <Show when={props.label}>
        <span class="mono chip">{props.label}</span>
      </Show>
    </span>
  );
}

export function EmptyState(props: { title: string; hint?: string; children?: JSX.Element }): JSX.Element {
  return (
    <div class="empty">
      <h3>{props.title}</h3>
      <Show when={props.hint}>
        <p>{props.hint}</p>
      </Show>
      <Show when={props.children}>
        <div class="row" style={{ "justify-content": "center", "margin-top": "16px" }}>
          {props.children}
        </div>
      </Show>
    </div>
  );
}

/** The signal meter. `live` animates the bars (respecting reduced-motion via CSS). */
export function Meter(props: { live: boolean }): JSX.Element {
  return (
    <div class={`meter ${props.live ? "live" : ""}`} aria-hidden="true">
      <For each={[0, 1, 2, 3, 4]}>{() => <span />}</For>
    </div>
  );
}
