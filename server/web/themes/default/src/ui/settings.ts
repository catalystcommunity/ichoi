import { h, button, field, select, section, heading, request, empty, modal, askConfirm, askText, note, link } from "./dom.ts";
import { useServers } from "../stores/servers.ts";
import { usePlayback } from "../stores/playback.ts";
import { useTheme, type ThemeMode } from "../stores/theme.ts";
import type { ServerApi } from "../lib/services.ts";
import type { ContentReportTargetType, Role } from "../lib/schema.ts";
import { isSignedIn, loadAllAccounts, loadAllBlockedAccounts, loadAllContentReports, deleteCurrentAccount, buildReportRequest, REPORT_REASONS, PRIVACY_URL, TERMS_URL } from "../lib/compliance.ts";

export function check(label: string, checked: boolean, change: (value: boolean) => unknown, disabled = false) {
  const input = h("input", { type: "checkbox", checked, disabled });
  input.addEventListener("change", () => { input.disabled = true; void Promise.resolve().then(() => change(input.checked)).catch(error => { input.checked = !input.checked; note(error); }).finally(() => { input.disabled = disabled; }); });
  return h("label", { class: "check" }, input, label);
}
export function serverBase(url: string): string { const parsed = new URL(url); parsed.protocol = parsed.protocol === "wss:" ? "https:" : "http:"; return parsed.origin; }
export function normalizedServer(value: string): string {
  const url = new URL(/^[a-z]+:\/\//i.test(value.trim()) ? value.trim() : `https://${value.trim()}`);
  if (!["https:", "http:", "wss:", "ws:"].includes(url.protocol) || url.username || url.password || url.search || url.hash || !["/", "/ws", ""].includes(url.pathname)) throw new Error("Enter a server domain or a server URL without a path.");
  url.protocol = url.protocol === "https:" || url.protocol === "wss:" ? "wss:" : "ws:"; url.pathname = "/ws"; return url.toString();
}
export function addServer(): void {
  const name = field("Server name"), address = field("Server address", "", "text");
  const body = h("div", { class: "stack" }, name, address);
  const dialog = modal("Add server", body);
  body.append(button("Connect", async () => {
    const id = await useServers().addServer(name.value, normalizedServer(address.value)); useServers().setActive(id); dialog.close();
  }, "primary"));
}
function account(api: ServerApi): HTMLElement {
  const servers = useServers(), current = servers.active()!, session = current.session;
  const body = section("Account", h("p", {}, session && isSignedIn(session) ? `Signed in as ${session.display_name ?? session.handle}` : "Sign in to access your library and saved progress."));
  if (isSignedIn(session)) body.append(h("div", { class: "actions" }, button("Sign out", () => servers.signOut()), button("Delete account", async () => {
    const entered = await askText({ title: "Delete account", label: `This deletes your account and all your playlists on ${current.name}. Enter ${session!.handle} to confirm.`, required: true, okLabel: "Delete account" });
    if (entered !== null) await deleteCurrentAccount(handle => api.session.deleteAccount({ confirmation_handle: handle }), servers.clearLocalSession, session!.handle, entered);
  }, "danger")));
  else if (new URL(current.url).host !== location.host) body.append(h("p", {}, "Open this server to sign in: ", link(current.name, serverBase(current.url))));
  else body.append(request(async () => {
    const response = await fetch("/api/auth", { signal: AbortSignal.timeout(15_000) }); if (!response.ok) throw new Error(await response.text());
    return await response.json() as { local_rp?: { start_url: string }; regular_rp?: { start_url: string } };
  }, config => {
    const provider = config.regular_rp ?? config.local_rp;
    if (!provider) return empty("Sign-in is not configured on this server");
    const identity = field("LinkKeys identity", "", "text");
    return h("div", { class: "stack" }, identity, button("Sign in with LinkKeys", async () => {
      if (!identity.value.trim()) throw new Error("Enter your LinkKeys identity.");
      const response = await fetch(provider.start_url, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ identity: identity.value.trim() }), signal: AbortSignal.timeout(15_000) });
      if (!response.ok) throw new Error(await response.text());
      const data = await response.json() as { redirect_url: string }; location.assign(data.redirect_url);
    }, "primary"));
  }));
  body.append(h("div", { class: "actions" }, link("Privacy policy", PRIVACY_URL), link("Terms of use", TERMS_URL)));
  return body;
}
export function reportContent(api: ServerApi, type: ContentReportTargetType, id: string): void {
  const reason = select("Report reason", REPORT_REASONS.map(reason => [reason, reason]), "objectionable-content", () => undefined);
  const details = h("textarea", { "aria-label": "Report details", maxLength: 2000, rows: 4 });
  const body = h("div", { class: "stack" }, h("label", {}, "Reason", reason), h("label", {}, "Details (optional)", details));
  const dialog = modal(`Report ${type}`, body);
  body.append(button("Send report", async () => { await api.library.reportContent(buildReportRequest(type, id, reason.value as typeof REPORT_REASONS[number], details.value)); dialog.close(); note("Report sent."); }, "primary"));
}
function reports(api: ServerApi) {
  const panel = request(() => loadAllContentReports(page => api.admin.listContentReports(page)), reports => {
    if (!reports.length) return empty("No content reports");
    return h("div", { class: "stack" }, ...reports.map(report => section(`${report.target_type} · ${report.reason}`,
      h("p", {}, report.details ?? "No details supplied"), h("p", { class: "muted" }, `${report.status} · ${report.target_id}`),
      h("div", { class: "actions" }, ...(["resolved", "dismissed"] as const).map(status => button(status === "resolved" ? "Resolve" : "Dismiss", async () => { await api.admin.updateContentReportStatus({ report_id: report.id, status }); await panel.refresh(); })),
        button("Delete reported content", async () => {
          if (report.target_type === "playlist") {
            if (!await askConfirm({ title: "Delete playlist?", text: "This removes the reported playlist from the server.", danger: true })) return;
            await api.library.deletePlaylist({ playlist_id: report.target_id });
          } else {
            // Do not make the report list wait for a separate account request.
            const target = (await loadAllAccounts(page => api.admin.listAccounts(page))).find(account => account.id === report.target_id);
            if (!target) throw new Error("This account is no longer available.");
            const entered = await askText({ title: "Delete reported account", label: `Enter ${target.handle} to delete the account and its playlists.`, required: true });
            if (entered === null) return;
            if (entered !== target.handle) throw new Error("The handle does not match.");
            await api.admin.deleteAccount({ account_id: target.id, confirmation_handle: entered });
          }
          await panel.refresh();
        }, "danger")))));
  });
  return section("Content reports", panel);
}
function accounts(api: ServerApi) {
  const panel = request(() => loadAllAccounts(page => api.admin.listAccounts(page)), accounts => h("div", { class: "stack" }, ...accounts.map(account => h("div", { class: "settings-row" },
    h("strong", {}, account.display_name ?? account.handle),
    select(`Role for ${account.handle}`, [["admin", "Administrator"], ["member", "Member"], ["guest", "Guest"]], account.role, async role => { await api.admin.setRole({ account_id: account.id, role: role as Role }); await panel.refresh(); }),
    check("Edit library artwork", account.can_edit_library ?? account.role === "admin", async enabled => { await api.admin.setLibraryEditor({ account_id: account.id, enabled }); }, account.role !== "member"),
    button("Report user", () => reportContent(api, "account", account.id), "flat")))));
  return section("Accounts", panel);
}
function trust(api: ServerApi) {
  const identity = field("Domain or handle at domain");
  const panel = request(() => api.admin.listTrustedIdentities(), result => h("div", { class: "stack" }, ...result.identities.map(entry => {
    const selector = entry.handle ? `${entry.handle}@${entry.domain}` : entry.domain;
    return h("div", { class: "settings-row" }, h("span", {}, selector), entry.source === "config" ? h("span", { class: "muted" }, "Deployment configuration") : button("Revoke", async () => { await api.admin.revokeTrustedIdentity({ identity: selector }); await panel.refresh(); }));
  })));
  return section("LinkKeys access", h("p", { class: "muted" }, "Trust a whole domain or one exact handle."), identity,
    button("Trust identity", async () => { if (!identity.value.trim()) throw new Error("Enter a domain or handle."); await api.admin.trustIdentity({ identity: identity.value.trim() }); identity.value = ""; await panel.refresh(); }), panel);
}
function satellites(api: ServerApi) {
  const panel = request(async () => {
    const [groups, satellites, nodes, accounts] = await Promise.all([api.admin.listGroups(), api.admin.listSatelliteTokens(), api.admin.listNodes(), loadAllAccounts(page => api.admin.listAccounts(page))]);
    return { groups: groups.groups, satellites: satellites.satellites, nodes: nodes.nodes, accounts };
  }, data => {
    const toggle = (values: string[], id: string, on: boolean) => on ? [...new Set([...values, id])] : values.filter(value => value !== id);
    const name = field("Satellite name"); let enabled = true, defaults = ["everyone"];
    const body = h("div", { class: "stack" }, name, check("Share newly discovered outputs", true, value => { enabled = value; }),
      h("div", { class: "checks" }, ...data.groups.map(group => check(group.name, defaults.includes(group.id), value => { defaults = toggle(defaults, group.id, value); }))),
      button("Generate token", async () => {
        const result = await api.admin.createNodeToken({ label: name.value, default_enabled: enabled, default_group_ids: defaults });
        modal("Copy this token now", h("div", { class: "stack" }, h("p", {}, "The server will not show this token again."), h("textarea", { readOnly: true, value: result.token, "aria-label": "Satellite token" })));
        await panel.refresh();
      }, "primary"), h("h3", {}, "Destinations"));
    for (const token of data.satellites) body.append(h("div", { class: "settings-row" }, h("strong", {}, token.name), button("Revoke", async () => {
      if (await askConfirm({ title: "Revoke satellite token?", text: `Disconnect ${token.name}?` })) { await api.admin.revokeSatelliteToken(token.id); await panel.refresh(); }
    })));
    const groupName = field("New access group");
    body.append(h("h3", {}, "Access groups"), groupName, button("Add group", async () => { if (!groupName.value.trim()) throw new Error("Enter a group name."); await api.admin.createGroup(groupName.value.trim()); await panel.refresh(); }));
    for (const group of data.groups) body.append(section(group.name, group.id === "everyone" ? h("p", {}, "Every guest and signed-in account.") : h("div", { class: "stack" },
      ...data.accounts.map(account => check(account.display_name ?? account.handle, group.member_account_ids.includes(account.id), async on => { await api.admin.setGroupMembers(group.id, toggle(group.member_account_ids, account.id, on)); await panel.refresh(); })),
      button("Delete group", async () => { await api.admin.deleteGroup(group.id); await panel.refresh(); }, "danger"))));
    body.append(h("h3", {}, "Discovered outputs"));
    for (const node of data.nodes) for (const device of node.devices) body.append(section(`${node.friendly_name} · ${device.friendly_name}`,
      check("Visible and controllable", device.enabled ?? true, async enabled => { await api.admin.setDeviceAccess({ device_id: device.id, enabled, group_ids: device.group_ids }); await panel.refresh(); }),
      h("div", { class: "checks" }, ...data.groups.map(group => check(group.name, device.group_ids.includes(group.id), async on => { await api.admin.setDeviceAccess({ device_id: device.id, enabled: device.enabled ?? true, group_ids: toggle(device.group_ids, group.id, on) }); await panel.refresh(); }, !device.enabled)))));
    return body;
  });
  return section("Satellite destinations and output access", panel);
}
export function settingsPage(): HTMLElement {
  const servers = useServers(), api = servers.api(), theme = useTheme(), pb = usePlayback(), admin = servers.active()?.session?.can_admin === true;
  const page = h("div", { class: "page settings-page" }, heading("Settings"));
  page.append(section("Servers", button("Add server", addServer), ...servers.servers.map(server => h("div", { class: "settings-row" },
    h("div", {}, h("strong", {}, server.name), h("p", { class: "muted" }, `${server.state}${server.detail ? ` · ${server.detail}` : ""}`)),
    button("Use server", () => servers.setActive(server.id)), button("Reconnect", () => servers.reconnect(server.id)),
    button("Remove", async () => { if (await askConfirm({ title: "Remove server?", text: `Remove ${server.name} from this browser?` })) servers.removeServer(server.id); }, "flat")))));
  if (api) page.append(account(api));
  page.append(section("Appearance", select("Color theme", [["system", "Use system theme"], ["light", "Light"], ["dark", "Dark"]], theme.mode(), value => theme.setMode(value as ThemeMode))));
  const bitrate = h("input", { type: "number", min: 0, value: String(pb.pref().max_bitrate_kbps ?? 0), "aria-label": "Maximum bitrate in kilobits per second" });
  bitrate.addEventListener("change", () => pb.setPref({ ...pb.pref(), max_bitrate_kbps: Math.max(0, Number(bitrate.value) || 0) }));
  page.append(section("Playback", select("Transcode format", [["aac", "AAC"], ["mp3", "MP3"]], pb.pref().transcode_codec ?? "aac", value => pb.setPref({ ...pb.pref(), transcode_codec: value as "aac" | "mp3" })),
    check("Prefer original audio", pb.pref().prefer_original ?? false, value => pb.setPref({ ...pb.pref(), prefer_original: value })), h("label", {}, "Maximum bitrate (0 means no limit)", bitrate)));
  if (api) {
    const settings = request(() => api.admin.getSettings(), data => {
      const mode = data.entries.theme; if (mode === "system" || mode === "light" || mode === "dark") theme.applyServerDefault(mode);
      return h("div", { class: "stack" }, ...Object.entries(data.entries).map(([key, value]) => {
        const input = field(key, value); input.disabled = !admin;
        return h("div", { class: "settings-row" }, input, admin ? button("Save", async () => { await api.admin.setSetting({ key, value: input.value }); note("Setting saved."); }) : null);
      }));
    });
    page.append(section("Server settings", settings));
    if (isSignedIn(servers.active()?.session)) {
      const blocked = request(() => loadAllBlockedAccounts(page => api.library.listBlockedAccounts(page)), accounts => accounts.length
        ? h("div", {}, ...accounts.map(account => h("div", { class: "settings-row" }, h("span", {}, account.display_name ?? account.handle), button("Unblock", async () => { await api.library.unblockAccount({ account_id: account.account_id }); await blocked.refresh(); }))))
        : empty("You have not blocked any accounts"));
      page.append(section("Blocked accounts", blocked));
    }
    if (admin) {
      const resync: import("./dom.ts").RequestPanel = request(() => api.admin.getResyncStatus(), status => h("div", { class: "actions" }, h("span", {}, status.running ? "Library scan is running" : "Library scan is idle"),
        button("Refresh status", () => resync.refresh()), button("Scan library", async () => { await api.admin.resyncLibrary(); await resync.refresh(); })));
      page.append(section("Library scan", resync), reports(api), accounts(api), trust(api), satellites(api));
    }
  }
  return page;
}
