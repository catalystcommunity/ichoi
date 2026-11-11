import { BaseComponent } from "@catalystcommunity/base-components";
import { type PushButton } from "@catalystcommunity/controls";
import { askConfirm, askText } from "@catalystcommunity/containers";
import { showNotice } from "@catalystcommunity/menus";
import { requestErrorMessage } from "../lib/request-error.ts";
export { BaseComponent, askConfirm, askText };
export type Child = Node | string | number | undefined | null | false;
export function h<K extends keyof HTMLElementTagNameMap>(tag: K, attrs: Record<string, unknown> = {}, ...children: Child[]): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs)) {
    if (key.startsWith("on") && typeof value === "function") element.addEventListener(key.slice(2).toLowerCase(), value as EventListener);
    else if (key === "class") element.className = String(value);
    else if (key in element && !key.startsWith("aria")) Reflect.set(element, key, value);
    else if (value !== undefined && value !== false && value !== null) element.setAttribute(key, String(value));
  }
  element.append(...children.filter((v): v is Node | string | number => v !== undefined && v !== null && v !== false).map(v => typeof v === "number" ? String(v) : v));
  return element;
}
export function note(message: unknown): void { showNotice({ text: message instanceof Error ? message.message : String(message), timeout: 8000 }); }
export function button(label: string, action: () => unknown, variant: PushButton["variant"] = "default"): PushButton {
  const control = h("push-button", { label, variant });
  control.addEventListener("click", () => {
    if (control.disabled) return;
    try {
      const result = action();
      if (result && typeof (result as Promise<unknown>).then === "function") {
        control.disabled = true;
        void Promise.resolve(result).catch(note).finally(() => { control.disabled = false; });
      }
    } catch (error) { note(error); }
  });
  return control;
}
export function link(label: string, href: string, className = ""): HTMLAnchorElement { return h("a", { href, class: className }, label); }
export function field(label: string, value = "", type = "text") {
  return h("text-field", { label, value, type });
}
export function select(label: string, options: readonly (readonly [string, string])[], value: string, change: (value: string) => unknown) {
  const input = h("select", { "aria-label": label }, ...options.map(([value, name]) => h("option", { value }, name)));
  input.value = value;
  input.addEventListener("change", () => { try { void Promise.resolve(change(input.value)).catch(note); } catch (error) { note(error); } });
  return input;
}
export function heading(title: string, subtitle?: string) { return h("header", { class: "page-head" }, h("h1", {}, title), subtitle ? h("p", { class: "muted" }, subtitle) : null); }
export function empty(title: string, help?: string) { return h("div", { class: "empty-state", role: "status" }, h("h3", {}, title), help ? h("p", {}, help) : null); }
export function loading(label = "Loading…") { return h("div", { class: "loading", role: "status" }, h("span", { class: "spinner", "aria-hidden": "true" }), label); }
export function requestError(error: unknown, retry: () => unknown) {
  const code = typeof error === "object" && error !== null && "code" in error ? error.code : undefined;
  return h("div", { class: "request-error", role: "alert" }, h("p", {}, requestErrorMessage(error)),
    h("div", { class: "actions" }, button("Retry", retry), code === 401 ? link("Sign in", "/settings") : null),
    h("details", {}, h("summary", {}, "Technical details"), h("pre", {}, error instanceof Error ? error.message : String(error))));
}
export function modal(title: string, content: Node) {
  const dialog = h("dialog-window", { label: title });
  dialog.append(content);
  document.body.append(dialog);
  void dialog.showModal().finally(() => dialog.remove());
  return dialog;
}

/** Each request owns its error state and ignores replies after replacement or disposal. */
export class RequestPanel extends BaseComponent {
  load: () => Promise<unknown> = async () => undefined;
  show: (value: any) => Node = value => h("p", {}, String(value));
  private generation = 0;
  connectedCallback(): void { super.connectedCallback(); void this.refresh(); }
  disconnectedCallback(): void { ++this.generation; super.disconnectedCallback(); }
  async refresh(): Promise<void> {
    const generation = ++this.generation;
    this.replaceChildren(loading());
    this.setAttribute("aria-busy", "true");
    try {
      const value = await this.load();
      if (this.isActive() && generation === this.generation) this.replaceChildren(this.show(value));
    } catch (error) {
      if (this.isActive() && generation === this.generation) this.replaceChildren(requestError(error, () => this.refresh()));
    } finally {
      if (generation === this.generation) this.setAttribute("aria-busy", "false");
    }
  }
}
customElements.define("ichoi-request", RequestPanel);
export function request<T>(load: () => Promise<T>, show: (value: T) => Node): RequestPanel {
  const panel = new RequestPanel(); panel.load = load; panel.show = show; return panel;
}
export function section(title: string, ...children: Child[]) { return h("section", { class: "panel" }, h("h2", {}, title), ...children); }
