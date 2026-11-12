import { BaseComponent, h, button, modal, request, empty } from "./dom.ts";
import { cachedCover } from "../lib/cover-art-cache.ts";
import type { ArtworkKind } from "../lib/schema.ts";
import type { ServerApi } from "../lib/services.ts";

export class Cover extends BaseComponent {
  api!: ServerApi;
  itemId = "";
  title = "";
  maxSize = 256;
  kind: ArtworkKind = "album";
  private url?: string;
  private generation = 0;
  connectedCallback(): void {
    super.connectedCallback();
    this.classList.add("cover");
    this.replaceChildren(h("span", { class: "cover-symbol", "aria-hidden": "true" }, this.kind === "artist" ? "♪" : "♫"), h("span", { class: "cover-title" }, this.title));
    const observer = new IntersectionObserver(entries => {
      if (entries.some(entry => entry.isIntersecting)) { observer.disconnect(); void this.load(); }
    }, { rootMargin: "80px" });
    observer.observe(this);
    this.own(() => { observer.disconnect(); ++this.generation; if (this.url) URL.revokeObjectURL(this.url); this.url = undefined; });
    this.own(cachedCover.watch(this.api, () => { void this.load(); }));
  }
  private async load(): Promise<void> {
    const generation = ++this.generation;
    try {
      const blob = await cachedCover(this.api, this.itemId, this.maxSize, this.kind);
      if (!this.isActive() || generation !== this.generation) return;
      if (this.url) URL.revokeObjectURL(this.url);
      this.url = URL.createObjectURL(blob);
      const image = h("img", { src: this.url, alt: `Cover art for ${this.title}`, loading: "lazy" });
      image.addEventListener("error", () => {
        if (!this.isActive() || generation !== this.generation) return;
        if (this.url) URL.revokeObjectURL(this.url);
        this.url = undefined;
        this.replaceChildren(h("span", { class: "cover-symbol", "aria-hidden": "true" }, this.kind === "artist" ? "♪" : "♫"), h("span", { class: "cover-title" }, this.title));
      }, { once: true });
      this.replaceChildren(image);
    } catch { /* Keep the title when artwork is absent. */ }
  }
}
customElements.define("ichoi-cover", Cover);
export function cover(api: ServerApi, id: string, title: string, kind: ArtworkKind = "album") {
  const element = new Cover(); Object.assign(element, { api, itemId: id, title, kind }); return element;
}
export function editArtwork(api: ServerApi, id: string, title: string, kind: ArtworkKind = "album"): void {
  let selected: string | undefined;
  const upload = h("input", { type: "file", accept: "image/jpeg,image/png,image/gif,image/webp", "aria-label": "Upload cover art" });
  const body = h("div", { class: "stack" }, h("p", {}, "This cover is shared by everyone on this server. Library scans keep your choice."));
  let offset = 0;
  const choices: import("./dom.ts").RequestPanel = request(() => api.library.listArtworkCandidates({ kind, id, offset, limit: 24 }), result => {
    const list = h("div", { class: "artwork-choices" });
    for (const candidate of result.candidates) {
      const choice = button(candidate.label, () => {
        selected = candidate.id; upload.value = "";
        list.querySelectorAll("push-button").forEach(control => { control.pressed = control === choice; });
      });
      choice.toggle = true;
      const preview = request(() => api.library.getArtworkCandidate({ kind, id, candidate_id: candidate.id, max_size: 256 }), art => {
        const url = URL.createObjectURL(new Blob([art.data as BlobPart], { type: art.content_type }));
        const img = h("img", { src: url, alt: candidate.label });
        img.addEventListener("load", () => URL.revokeObjectURL(url), { once: true });
        return img;
      });
      list.append(h("div", {}, preview, choice));
    }
    return h("div", {}, list.childElementCount ? list : empty("No cover choices"), h("div", { class: "actions" },
      offset ? button("Previous choices", () => { offset = Math.max(0, offset - 24); return choices.refresh(); }) : null,
      result.candidates.length === 24 ? button("More choices", () => { offset += 24; return choices.refresh(); }) : null));
  });
  body.append(choices, h("label", { class: "field" }, "Upload an image (up to 8 MB)", upload));
  upload.addEventListener("change", () => { selected = undefined; });
  const dialog = modal(`Cover art: ${title}`, body);
  const save = async (reset: boolean) => {
    const file = upload.files?.[0];
    if (!reset && !file && !selected) throw new Error("Choose a cover first.");
    if (file && file.size > 8 * 1024 * 1024) throw new Error("Choose an image no larger than 8 MB.");
    await api.library.setArtwork({ kind, id, candidate_id: reset ? undefined : selected,
      data: !reset && file ? new Uint8Array(await file.arrayBuffer()) : undefined });
    cachedCover.invalidate(api); dialog.close();
  };
  body.append(h("div", { class: "actions" }, button("Use automatic cover", () => save(true)), button("Save cover", () => save(false), "primary")));
}
