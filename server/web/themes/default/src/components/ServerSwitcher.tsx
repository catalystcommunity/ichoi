// The multi-server switcher in the nav rail (DESIGN §7). Lists connected servers
// with a live status lamp, lets you switch the active one, add another, and kick
// off (stubbed) LinkKeys sign-in. Each server is a separate session.
import { createSignal, For, Show, type JSX } from "solid-js";
import { useI18n } from "../lib/i18n.tsx";
import { useServers } from "../stores/servers.tsx";
import { Dialog } from "./Dialog.tsx";
import { IconPlus, IconBroadcast } from "./Icons.tsx";

export function ServerSwitcher(): JSX.Element {
  const servers = useServers();
  const { t } = useI18n();
  const [showAdd, setShowAdd] = createSignal(false);
  const [name, setName] = createSignal("");
  const [url, setUrl] = createSignal("");

  const submit = async (e: Event) => {
    e.preventDefault();
    if (!url().trim()) return;
    await servers.addServer(name(), url());
    setShowAdd(false);
    setName("");
    setUrl("");
  };

  return (
    <section class="servers" aria-label={t("servers.title")}>
      <div class="servers-head">
        <span class="eyebrow">{t("servers.title")}</span>
        <button
          type="button"
          class="btn btn-ghost btn-icon"
          aria-label={t("servers.add")}
          onClick={() => setShowAdd(true)}
        >
          <IconPlus size={16} />
        </button>
      </div>

      <ul role="list">
        <For each={servers.servers}>
          {(server) => (
            <li>
              <button
                type="button"
                class="server-item"
                aria-current={server.id === servers.activeId() ? "true" : "false"}
                onClick={() => servers.setActive(server.id)}
              >
                <span
                  class={`server-dot ${server.state}`}
                  aria-hidden="true"
                  title={t(`servers.state.${server.state}` as never)}
                />
                <span class="server-meta">
                  <span class="server-name">{server.name}</span>
                  <span class="server-state">
                    {t(`servers.state.${server.state}` as never)}
                    <Show when={server.session}>{` · ${server.session!.handle}`}</Show>
                  </span>
                </span>
              </button>
            </li>
          )}
        </For>
      </ul>

      <AuthArea />

      <Dialog open={showAdd()} title={t("servers.addTitle")} onClose={() => setShowAdd(false)}>
        <form onSubmit={submit}>
          <div class="field">
            <label for="srv-name">{t("servers.name")}</label>
            <input
              id="srv-name"
              class="input"
              value={name()}
              placeholder={t("servers.namePlaceholder")}
              onInput={(e) => setName(e.currentTarget.value)}
            />
          </div>
          <div class="field">
            <label for="srv-url">{t("servers.url")}</label>
            <input
              id="srv-url"
              class="input"
              required
              value={url()}
              placeholder={t("servers.urlPlaceholder")}
              onInput={(e) => setUrl(e.currentTarget.value)}
            />
          </div>
          <div class="dialog-actions">
            <button type="button" class="btn btn-ghost" onClick={() => setShowAdd(false)}>
              {t("servers.cancel")}
            </button>
            <button type="submit" class="btn btn-primary">
              {t("servers.connect")}
            </button>
          </div>
        </form>
      </Dialog>
    </section>
  );
}

/** Sign-in area. LinkKeys is the real identity path (§8); the handshake is stubbed
 * with a clear TODO. Login-less connections are already guests, so this only
 * *upgrades* identity. */
function AuthArea(): JSX.Element {
  const servers = useServers();
  const { t } = useI18n();
  const session = () => servers.active()?.session;

  const startLinkKeys = () => {
    // TODO(linkkeys): kick off the LinkKeys RP flow.
    //   1. Redirect/pop to the configured LinkKeys RP authorize endpoint.
    //   2. Receive the sealed assertion (the `linkkeys_assertion` bytes).
    //   3. Call SessionService.authenticate({ linkkeys_assertion }) on the active
    //      server, store the returned `token`, and reconnect with it as Hello.auth.
    // The server, DB, and CSIL surface for this exist; only the RP handshake and
    // token persistence remain to wire up here.
    alert(t("auth.linkkeysTodo"));
  };

  return (
    <div style={{ "margin-top": "12px" }}>
      <Show
        when={session() && session()!.role !== "guest"}
        fallback={
          <button type="button" class="btn" style={{ width: "100%" }} onClick={startLinkKeys}>
            <IconBroadcast size={16} /> {t("auth.signIn")}
          </button>
        }
      >
        <div class="chip" style={{ "line-height": "1.6" }}>
          {t("auth.signedInAs", { handle: session()!.display_name ?? session()!.handle })}
          <br />
          <span class="badge role">{session()!.role}</span>
        </div>
      </Show>
    </div>
  );
}
