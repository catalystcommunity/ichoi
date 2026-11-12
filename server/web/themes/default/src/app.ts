import SimpleRouter from "@catalystcommunity/simple-router";
import { BaseComponent, h, link, select, button, requestError, loading } from "./ui/dom.ts";
import { LibraryPage } from "./ui/catalog.ts";
import { PlayerBar, QueuePanel, satellitePage } from "./ui/player.ts";
import { detailPage, playlistsPage } from "./ui/details.ts";
import { devicesPage } from "./ui/devices.ts";
import { settingsPage } from "./ui/settings.ts";
import { useServers } from "./stores/servers.ts";
import { usePlayback } from "./stores/playback.ts";
import { useTheme } from "./stores/theme.ts";
import { observe } from "./lib/state.ts";
import { satelliteToken } from "./lib/satellite-mode.ts";

export class IchoiApp extends BaseComponent {
  private main = h("main", { id: "main-content", class: "main-content", tabIndex: -1 });
  private nav = h("nav", { class: "navigation", "aria-label": "Primary" });
  private servers = select("Active server", [], "", value => useServers().setActive(value));
  private status = h("div", { class: "connection-status", role: "status" });
  private player = new PlayerBar();
  private queueHost = h("dialog", { class: "queue-host", "aria-label": "Playback queue" });
  private sidebar = h("aside", { class: "sidebar", "aria-label": "Library navigation" });
  private sidebarSlot = h("div", { class: "sidebar-slot" });
  private menu = h("dialog", { id: "navigation-drawer", class: "navigation-drawer", "aria-label": "Navigation" });
  private menuButton = h("button", { type: "button", class: "shell-button", "aria-controls": "navigation-drawer", "aria-expanded": "false", onclick: () => this.openMenu() }, "☰ Menu");
  private narrow = matchMedia("(max-width: 900px)");
  private router = new SimpleRouter();
  private queueOpen = false;
  private opener?: HTMLElement;
  private initialized = false;
  connectedCallback(): void {
    super.connectedCallback(); useTheme(); usePlayback(); this.className = "app-shell";
    this.nav.replaceChildren(...[["/", "Library"], ["/audiobooks", "Audiobooks"], ["/search", "Search"], ["/playlists", "Playlists"], ["/jukebox", "Devices"], ["/settings", "Settings"]].filter(([path]) => !satelliteToken() || !["/jukebox", "/settings"].includes(path!)).map(([path, label]) => link(label!, path!)));
    if (satelliteToken()) this.nav.append(link("Satellite", "/satellite"));
    const queue = new QueuePanel(); queue.close = () => this.toggleQueue(false); this.queueHost.append(queue);
    this.player.openQueue = () => this.toggleQueue(!this.queueOpen);
    const brand = () => h("a", { href: "/", class: "brand", "aria-label": "Ichoi library" }, h("img", { src: "/brand/ichoi-mark.svg", alt: "" }), h("span", {}, "ichoi"));
    this.sidebar.replaceChildren(h("div", { class: "sidebar-heading" }, brand(), h("button", { type: "button", class: "shell-button menu-close", onclick: () => this.closeMenu() }, "Close")), this.nav,
      h("label", { class: "sidebar-server" }, h("span", { class: "eyebrow" }, "Your server"), this.servers));
    this.sidebarSlot.append(this.sidebar);
    this.replaceChildren(link("Skip to content", "#main-content", "skip-link"),
      h("header", { class: "app-header" }, this.menuButton, brand()), this.sidebarSlot,
      this.status, h("div", { class: "workspace" }, this.main, this.queueHost), this.player, this.menu);
    this.listen(this.menu, "cancel", (event: Event) => { event.preventDefault(); this.closeMenu(); });
    this.listen(this.menu, "click", (event: MouseEvent) => { if (event.target === this.menu) this.closeMenu(); });
    this.listen(this.queueHost, "cancel", (event: Event) => { event.preventDefault(); this.toggleQueue(false); });
    this.listen(this.queueHost, "click", (event: MouseEvent) => { if (event.target === this.queueHost) this.toggleQueue(false); });
    this.listen(this.narrow, "change", () => {
      this.closeMenu();
      if (this.queueOpen) { this.queueHost.close(); this.showQueue(); }
    });
    const route = () => { if (this.initialized) this.route(); };
    for (const path of ["/", "/audiobooks", "/search", "/album", "/artist", "/audiobook", "/source", "/playlists", "/jukebox", "/settings", "/satellite", "/now-playing"]) this.router.registerRoute(path, route, [], path === "/");
    this.listen(this, "click", (event: MouseEvent) => {
      const anchor = (event.target as Element)?.closest<HTMLAnchorElement>("a[href]");
      if (!anchor || event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey || anchor.target || anchor.hasAttribute("download")) return;
      const url = new URL(anchor.href);
      if (url.origin !== location.origin || anchor.getAttribute("href")?.startsWith("#")) return;
      event.preventDefault(); const fromMenu = this.menu.open; this.closeMenu(); this.router.changePage(url.pathname + url.search + url.hash); if (fromMenu) this.main.focus();
    });
    this.listen(document, "keydown", (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.defaultPrevented && !document.querySelector("dialog:modal") && this.queueOpen) this.toggleQueue(false);
      if ((event.ctrlKey || event.metaKey) && event.key === "k") { event.preventDefault(); this.closeMenu(); this.toggleQueue(false); this.router.changePage("/search"); this.main.querySelector<HTMLInputElement>("input[type=search]")?.focus(); }
    });
    this.own(useServers().subscribe(() => this.invalidate()));
    this.own(observe([useServers()], () => [useServers().api(), useServers().activeId(), useServers().active()?.session?.account_id, useServers().active()?.session?.can_admin], () => this.route()));
    this.own(() => this.router.cleanup());
    this.listen(window, "popstate", () => { this.closeMenu(); this.toggleQueue(false); });
    this.listen(window, "ichoi:update-reloading", () => { this.status.replaceChildren(loading("Updating Ichoi…")); this.status.hidden = false; });
    this.initialized = true; this.route(); this.invalidate();
  }
  private openMenu(): void {
    if (!this.narrow.matches || this.menu.open) return;
    this.toggleQueue(false);
    this.menu.append(this.sidebar);
    this.menu.showModal();
    this.menuButton.setAttribute("aria-expanded", "true");
  }
  private closeMenu(): void {
    if (!this.menu.open) return;
    this.menu.close();
    this.sidebarSlot.append(this.sidebar);
    this.menuButton.setAttribute("aria-expanded", "false");
    if (this.narrow.matches) this.menuButton.focus();
  }
  private showQueue(): void {
    if (this.narrow.matches) this.queueHost.showModal(); else this.queueHost.show();
  }
  private toggleQueue(open: boolean): void {
    if (open === this.queueOpen) return;
    if (open) this.closeMenu();
    this.queueOpen = open; this.classList.toggle("queue-open", open);
    if (open) { this.opener = document.activeElement as HTMLElement; this.showQueue(); this.queueHost.querySelector<HTMLElement>("button")?.focus(); }
    else { this.queueHost.close(); if (this.opener?.isConnected) this.opener.focus(); }
  }
  private route(): void {
    if (!this.isActive()) return;
    let path = location.pathname;
    if (path === "/now-playing") {
      this.toggleQueue(true);
      if (this.main.childElementCount) return;
      path = "/";
    }
    for (const anchor of this.nav.querySelectorAll("a")) {
      if (new URL(anchor.href).pathname === path) anchor.setAttribute("aria-current", "page"); else anchor.removeAttribute("aria-current");
    }
    const servers = useServers();
    if (!servers.api() && path !== "/settings" && path !== "/satellite") { this.main.replaceChildren(loading("Connecting to your library…")); return; }
    try {
      const parts = path.split("/").filter(Boolean).map(decodeURIComponent);
      let page: Node;
      if (["/", "/audiobooks", "/search"].includes(path)) {
        const library = new LibraryPage(); library.library = path === "/audiobooks" ? "audiobook" : "music"; library.searchOnly = path === "/search"; library.initialQuery = new URLSearchParams(location.search).get("q") ?? ""; page = library;
      } else if (parts[0] === "source" && parts.length === 5) page = detailPage(parts[3] === "artist" ? "artist" : "album", parts[4]!, parts[2] === "audiobook" ? "audiobook" : "music", parts[1]);
      else if (["album", "artist", "audiobook"].includes(parts[0] ?? "") && parts[1]) page = detailPage(parts[0] === "artist" ? "artist" : "album", parts[1], parts[0] === "audiobook" ? "audiobook" : "music");
      else if (path === "/settings" && !satelliteToken()) page = settingsPage();
      else if (path === "/playlists") page = playlistsPage();
      else if (path === "/jukebox" && !satelliteToken()) page = devicesPage();
      else if (path === "/satellite") page = satellitePage();
      else page = new LibraryPage();
      this.main.replaceChildren(page); this.main.scrollTop = 0;
    } catch (error) { this.main.replaceChildren(requestError(error, () => this.route())); }
  }
  render(): void {
    const servers = useServers(), active = servers.active();
    const selected = servers.activeId() ?? "";
    this.servers.replaceChildren(...servers.servers.map(server => h("option", { value: server.id }, server.name))); this.servers.value = selected;
    const failed = active?.state === "error" || active?.state === "closed";
    this.status.hidden = !failed;
    if (failed) this.status.replaceChildren(h("span", {}, "Connection interrupted. Your queue and loaded library remain available."), button("Reconnect", () => servers.reconnect(active!.id)));
  }
}
customElements.define("ichoi-app", IchoiApp);
