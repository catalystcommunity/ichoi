import { ArtworkChoices } from "./ArtworkChoices.tsx";
import { createEffect, createResource, createSignal, on, onCleanup, Show, type JSX } from "solid-js";
import type { Album, ArtworkKind } from "../lib/schema.ts";
import { cachedCover } from "../lib/cover-art-cache.ts";
import { useServers } from "../stores/servers.tsx";
import { Dialog } from "./Dialog.tsx";

interface Props {
  album: Pick<Album, "id" | "title"> & { has_cover_art?: boolean };
  kind?: ArtworkKind;
  maxSize?: number;
  class?: string;
  serverId?: string;
}

export function CoverArt(props: Props): JSX.Element {
  const servers = useServers();
  const api = () => props.serverId ? servers.apiFor(props.serverId) : servers.api();
  const server = () => props.serverId ? servers.servers.find(s => s.id === props.serverId) : servers.active();
  const [revision, setRevision] = createSignal(0);
  const [open, setOpen] = createSignal(false);
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string>();
  const [preview, setPreview] = createSignal<string>();
  const [candidate, setCandidate] = createSignal<string>();
  const [file, setFile] = createSignal<File>();
  let generation = 0;
  let url: string | undefined;
  const label = () => /[\p{L}\p{N}]/u.test(props.album.title) ? props.album.title.trim() : (props.kind === "artist" ? "Unknown artist" : "Untitled album");
  createEffect(on(() => [props.album.id, props.kind, server()?.id], () => {
    setOpen(false); setFile(undefined); setCandidate(undefined); setError(undefined);
    if (preview()) URL.revokeObjectURL(preview()!);
    setPreview(undefined);
  }));
  createEffect(() => {
    server()?.state;
    const client = api();
    if (client) onCleanup(cachedCover.watch(client, () => setRevision(v => v + 1)));
  });
  const [image] = createResource(
    () => { server()?.state; return { api: api(), id: props.album.id, kind: props.kind ?? "album", size: props.maxSize ?? 512, revision: revision() }; },
    async input => {
      const request = ++generation;
      if (url) URL.revokeObjectURL(url);
      url = undefined;
      if (!input.api) return undefined;
      try {
        const blob = await cachedCover(input.api, input.id, input.size, input.kind);
        if (request !== generation) return undefined;
        url = URL.createObjectURL(blob);
        return url;
      } catch { return undefined; }
    },
  );
  onCleanup(() => { generation++; if (url) URL.revokeObjectURL(url); if (preview()) URL.revokeObjectURL(preview()!); });
  function close() { if (!busy()) { setOpen(false); setFile(undefined); setCandidate(undefined); if (preview()) URL.revokeObjectURL(preview()!); setPreview(undefined); } }
  function choose(file?: File) {
    setError(undefined);
    setCandidate(undefined);
    setFile(undefined);
    if (preview()) URL.revokeObjectURL(preview()!);
    setPreview(undefined);
    if (!file) return;
    if (file.size > 8 * 1024 * 1024) { setError("Choose an image no larger than 8 MB."); return; }
    setFile(file); setPreview(URL.createObjectURL(file));
  }
  async function save(reset = false) {
    const client = api();
    if (!client || busy() || (!reset && !file() && !candidate())) return;
    const target = { kind: props.kind ?? "album", id: props.album.id } as const;
    setBusy(true); setError(undefined);
    try {
      const data = reset || !file() ? undefined : new Uint8Array(await file()!.arrayBuffer());
      await client.library.setArtwork({ ...target, data, candidate_id: reset ? undefined : candidate() });
      cachedCover.invalidate(client);
      setBusy(false); close();
    } catch (error) { setError(String(error)); }
    finally { setBusy(false); }
  }
  return <>
    <div class={`cover ${props.class ?? ""}`} title="Right-click or press Shift+F10 to choose cover art"
      onContextMenu={event => { event.preventDefault(); event.stopPropagation(); setError(undefined); setOpen(true); }}
      onKeyDown={event => { if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) { event.preventDefault(); event.stopPropagation(); setOpen(true); } }}
      tabIndex={0} aria-label={`Cover art for ${props.album.title}`}>
      <Show when={!image.loading && image()} fallback={<span class="cover-fallback"><span class="cover-symbol" aria-hidden="true">♫</span><span>{label()}</span></span>}>
        <img src={image()} alt={`Cover art for ${props.album.title}`} loading="lazy" />
      </Show>
    </div>
    <Dialog open={open()} title={`Cover art: ${props.album.title}`} onClose={close}>
      <div onClick={event => event.stopPropagation()}>
        <Show when={server()?.session?.can_edit_library === true || server()?.session?.role === "admin"} fallback={<p>An administrator must grant you “Edit library artwork” permission to change shared covers.</p>}>
          <p>This cover is shared by everyone on this server. Library scans will keep your choice.</p>
          <Show when={open() && api()}>{client => <ArtworkChoices api={client()} kind={props.kind ?? "album"} id={props.album.id}
            selected={candidate()} busy={busy()} onChoose={id => { choose(undefined); setCandidate(id); }} />}</Show>
          <label class="field">Upload an image (up to 8 MB)
            <input type="file" accept="image/jpeg,image/png,image/gif,image/webp" disabled={busy()} onChange={event => choose(event.currentTarget.files?.[0])} />
          </label>
          <Show when={preview()}><img class="cover-preview" src={preview()} alt="Selected cover preview" /></Show>
          <Show when={error()}><p class="error" role="alert">{error()}</p></Show>
          <div class="dialog-actions">
            <button type="button" class="btn" disabled={busy()} onClick={() => void save(true)}>Use automatic cover</button>
            <button type="button" class="btn btn-primary" disabled={busy() || (!file() && !candidate())} onClick={() => void save()}>{busy() ? "Saving…" : "Save cover"}</button>
          </div>
        </Show>
        <button type="button" class="btn btn-ghost" disabled={busy()} onClick={close}>Close</button>
      </div>
    </Dialog>
  </>;
}
