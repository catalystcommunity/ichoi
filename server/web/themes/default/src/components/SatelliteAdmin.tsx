import { createResource, createSignal, For, Show, type JSX } from "solid-js";
import { useServers } from "../stores/servers.tsx";

export function SatelliteAdmin(): JSX.Element {
  const servers = useServers();
  const [name, setName] = createSignal("");
  const [newGroup, setNewGroup] = createSignal("");
  const [defaults, setDefaults] = createSignal<string[]>(["everyone"]);
  const [defaultEnabled, setDefaultEnabled] = createSignal(true);
  const [revealedToken, setRevealedToken] = createSignal<string>();
  const [message, setMessage] = createSignal<string>();

  const [data, { refetch }] = createResource(
    () => servers.active()?.session?.can_admin ? servers.api() : undefined,
    async (api) => {
      const [accounts, groups, satellites, nodes] = await Promise.all([
        api!.admin.listAccounts(),
        api!.admin.listGroups(),
        api!.admin.listSatelliteTokens(),
        api!.admin.listNodes(),
      ]);
      return { accounts: accounts.accounts, groups: groups.groups, satellites: satellites.satellites, nodes: nodes.nodes };
    },
  );

  const toggle = (values: string[], id: string, checked: boolean) =>
    checked ? [...new Set([...values, id])] : values.filter((value) => value !== id);

  async function createSatellite(event: Event): Promise<void> {
    event.preventDefault();
    const api = servers.api();
    if (!api) return;
    setMessage(undefined);
    try {
      const result = await api.admin.createNodeToken({
        label: name(),
        default_enabled: defaultEnabled(),
        default_group_ids: defaults(),
      });
      setRevealedToken(result.token);
      setName("");
      await refetch();
    } catch (cause) {
      setMessage(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function createAccessGroup(event: Event): Promise<void> {
    event.preventDefault();
    const api = servers.api();
    if (!api || !newGroup().trim()) return;
    try {
      await api.admin.createGroup(newGroup().trim());
      setNewGroup("");
      await refetch();
    } catch (cause) {
      setMessage(cause instanceof Error ? cause.message : String(cause));
    }
  }

  return (
    <section class="panel" aria-labelledby="satellite-admin-title">
      <h2 id="satellite-admin-title">Satellite destinations and output access</h2>
      <p class="hint">Tokens bind a PWA or native satellite to a named destination. New outputs inherit the token defaults.</p>

      <Show when={revealedToken()}>{(token) => (
        <div class="field token-reveal">
          <label>Copy this token now; it will not be shown again.</label>
          <code class="mono">{token()}</code>
        </div>
      )}</Show>

      <form onSubmit={(event) => void createSatellite(event)} class="field">
        <label for="satellite-name">New satellite destination</label>
        <div class="row">
          <input id="satellite-name" class="input" required maxlength={64} placeholder="Kitchen Chromebook" value={name()} onInput={(event) => setName(event.currentTarget.value)} />
          <button class="btn btn-primary" type="submit">Generate token</button>
        </div>
        <label class="row"><input type="checkbox" checked={defaultEnabled()} onChange={(event) => setDefaultEnabled(event.currentTarget.checked)} /> Share newly discovered outputs</label>
        <div class="check-grid">
          <For each={data()?.groups ?? []}>{(group) => (
            <label class="row">
              <input type="checkbox" checked={defaults().includes(group.id)} onChange={(event) => setDefaults((current) => toggle(current, group.id, event.currentTarget.checked))} />
              {group.name}
            </label>
          )}</For>
        </div>
      </form>

      <h3>Destinations</h3>
      <div class="settings-list">
        <For each={data()?.satellites ?? []}>{(satellite) => (
          <div class="row spread settings-row">
            <span><strong>{satellite.name}</strong><br /><span class="hint">Defaults: {satellite.default_enabled ? "shared" : "disabled"} · {satellite.default_group_ids.map((id) => data()?.groups.find((group) => group.id === id)?.name ?? id).join(", ") || "no groups"}</span></span>
            <button class="btn btn-ghost" type="button" onClick={async () => { await servers.api()?.admin.revokeSatelliteToken(satellite.id); await refetch(); }}>Revoke</button>
          </div>
        )}</For>
      </div>

      <h3>Access groups</h3>
      <form class="row" onSubmit={(event) => void createAccessGroup(event)}>
        <input class="input" placeholder="Household" maxlength={64} value={newGroup()} onInput={(event) => setNewGroup(event.currentTarget.value)} />
        <button class="btn" type="submit">Add group</button>
      </form>
      <div class="settings-list">
        <For each={data()?.groups ?? []}>{(group) => (
          <div class="settings-row">
            <div class="row spread"><strong>{group.name}</strong><Show when={group.id !== "everyone"}><button class="btn btn-ghost" type="button" onClick={async () => { await servers.api()?.admin.deleteGroup(group.id); await refetch(); }}>Delete</button></Show></div>
            <Show when={group.id !== "everyone"} fallback={<p class="hint">Every guest and signed-in account.</p>}>
              <div class="check-grid">
                <For each={data()?.accounts ?? []}>{(account) => (
                  <label class="row"><input type="checkbox" checked={group.member_account_ids.includes(account.id)} onChange={async (event) => { await servers.api()?.admin.setGroupMembers(group.id, toggle(group.member_account_ids, account.id, event.currentTarget.checked)); await refetch(); }} />{account.display_name ?? account.handle}</label>
                )}</For>
              </div>
            </Show>
          </div>
        )}</For>
      </div>

      <h3>Discovered outputs</h3>
      <div class="settings-list">
        <For each={data()?.nodes.flatMap((node) => node.devices.map((device) => ({ node, device }))) ?? []}>{({ node, device }) => (
          <div class="settings-row">
            <div><strong>{node.friendly_name} · {device.friendly_name}</strong></div>
            <label class="row"><input type="checkbox" checked={device.enabled ?? true} onChange={async (event) => { await servers.api()?.admin.setDeviceAccess({ device_id: device.id, enabled: event.currentTarget.checked, group_ids: device.group_ids }); await refetch(); }} />Visible and controllable</label>
            <div class="check-grid">
              <For each={data()?.groups ?? []}>{(group) => (
                <label class="row"><input type="checkbox" checked={device.group_ids.includes(group.id)} disabled={!device.enabled} onChange={async (event) => { await servers.api()?.admin.setDeviceAccess({ device_id: device.id, enabled: device.enabled ?? true, group_ids: toggle(device.group_ids, group.id, event.currentTarget.checked) }); await refetch(); }} />{group.name}</label>
              )}</For>
            </div>
          </div>
        )}</For>
      </div>
      <Show when={message()}>{(text) => <p class="error">{text()}</p>}</Show>
    </section>
  );
}
