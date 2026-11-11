import { createResource, createSignal, For, onCleanup, Show, type JSX } from "solid-js";
import type { ArtworkCandidate, ArtworkKind } from "../lib/schema.ts";
import type { ServerApi } from "../lib/services.ts";

export function ArtworkChoices(props: { api: ServerApi; kind: ArtworkKind; id: string; selected?: string; busy: boolean; onChoose: (id: string) => void }): JSX.Element {
  const [offset, setOffset] = createSignal(0);
  const [error, setError] = createSignal<string>();
  const [page] = createResource(() => ({ api: props.api, kind: props.kind, id: props.id, offset: offset(), limit: 12 }), async request => {
    setError(undefined);
    try { return await request.api.library.listArtworkCandidates(request); }
    catch (error) { setError(`Could not load library images: ${String(error)}`); return undefined; }
  });
  return <section aria-label="Images already in this library">
    <h3>Images in this library</h3>
    <Show when={page.loading}><p role="status">Loading images…</p></Show>
    <Show when={error()}><p class="error" role="alert">{error()}</p></Show>
    <Show when={!page.loading && page()?.total === 0}><p>No local images found. You can upload a cover below.</p></Show>
    <div class="artwork-choices">
      <For each={page()?.candidates}>{candidate => <button type="button" class="artwork-choice"
        aria-pressed={props.selected === candidate.id} disabled={props.busy} onClick={() => props.onChoose(candidate.id)}>
        <CandidatePreview api={props.api} kind={props.kind} id={props.id} candidate={candidate} />
        <span>{candidate.label}</span><small>{candidate.width} × {candidate.height} · {candidate.role}{candidate.suggested ? " · Automatic choice" : ""}</small>
      </button>}</For>
    </div>
    <Show when={(page()?.total ?? 0) > 12}><div class="row spread">
      <button type="button" class="btn" disabled={props.busy || offset() === 0} onClick={() => setOffset(value => Math.max(0,value-12))}>Previous images</button>
      <button type="button" class="btn" disabled={props.busy || offset()+12 >= (page()?.total ?? 0)} onClick={() => setOffset(value => value+12)}>More images</button>
    </div></Show>
  </section>;
}
function CandidatePreview(props: { api: ServerApi; kind: ArtworkKind; id: string; candidate: ArtworkCandidate }): JSX.Element {
  let url: string | undefined;
  let disposed = false;
  const [image] = createResource(async () => {
    try {
      const cover = await props.api.library.getArtworkCandidate({ kind: props.kind, id: props.id, candidate_id: props.candidate.id, max_size: 160 });
      if (disposed) return undefined;
      url = URL.createObjectURL(new Blob([cover.data as BlobPart], {type: cover.content_type}));
      return url;
    } catch { return undefined; }
  });
  onCleanup(() => { disposed = true; if (url) URL.revokeObjectURL(url); });
  return <Show when={image()} fallback={<span>Preview unavailable</span>}><img src={image()} alt={props.candidate.label} /></Show>;
}
