//! CSIL surface dispatch and framing.
//!
//! Two entry points share one `dispatch` table:
//! - `handle_events_frame` — the **real CSIL-Events wire** (verbose profile), one binary
//!   WebSocket frame per envelope, matching the web UI. This is what browsers use.
//! - `handle_json` — a text JSON envelope (payload still canonical CBOR) kept for shell-level
//!   debugging over the TCP port.
//!
//! Channel operations are routed through the owning transport: browser/player subscriptions
//! fan out state frames, and native TCP satellites use node directives plus media stream
//! effects returned from `handle_events_frame`.

use ciborium::value::Value;
use libichoi::csil::codec::*;
use libichoi::csil::services::*;
use libichoi::csil::types::{MediaControl, MediaOpen, PlayerState, ServiceError};
use libichoi::csil_channel::decode_media_control;
use serde::{Deserialize, Serialize};

use crate::handlers::{App, Ctx, Identity};

#[derive(Deserialize)]
struct Envelope {
    service: String,
    op: String,
    #[serde(default)]
    id: u64,
    #[serde(default)]
    payload_hex: String,
}

#[derive(Serialize)]
struct Reply {
    id: u64,
    status: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload_hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn bad(e: CsilCborError) -> ServiceError {
    ServiceError {
        code: 400,
        message: format!("malformed payload: {}", e.0),
    }
}

fn reply_json(id: u64, result: Result<Vec<u8>, ServiceError>) -> String {
    let reply = match result {
        Ok(payload) => Reply {
            id,
            status: 0,
            payload_hex: Some(hex::encode(payload)),
            error: None,
        },
        Err(e) => Reply {
            id,
            status: e.code,
            payload_hex: None,
            error: Some(e.message),
        },
    };
    serde_json::to_string(&reply).unwrap_or_else(|_| "{\"id\":0,\"status\":500}".to_string())
}

/// Handle one JSON envelope, returning the JSON reply.
pub fn handle_json(app: &App, ctx: &Ctx, text: &str) -> String {
    let env: Envelope = match serde_json::from_str(text) {
        Ok(e) => e,
        Err(e) => {
            return reply_json(
                0,
                Err(ServiceError {
                    code: 400,
                    message: format!("bad envelope: {e}"),
                }),
            )
        }
    };
    let payload = match hex::decode(&env.payload_hex) {
        Ok(p) => p,
        Err(_) => {
            return reply_json(
                env.id,
                Err(ServiceError {
                    code: 400,
                    message: "payload_hex not valid hex".to_string(),
                }),
            )
        }
    };
    reply_json(env.id, dispatch(app, ctx, &env.service, &env.op, &payload))
}

/// Route (service, op) → decode → handler → encode. Every request/response operation.
pub fn dispatch(
    app: &App,
    ctx: &Ctx,
    service: &str,
    op: &str,
    payload: &[u8],
) -> Result<Vec<u8>, ServiceError> {
    macro_rules! rr {
        ($decode:path, $method:ident, $encode:path) => {{
            let input = $decode(payload).map_err(bad)?;
            let out = app.$method(ctx, input)?;
            Ok($encode(&out))
        }};
    }

    // The client may name the service either fully (`LibraryService`) or stripped
    // (`library`); normalize to the stripped, lowercased form the arms below use.
    let svc = service
        .strip_suffix("Service")
        .unwrap_or(service)
        .to_ascii_lowercase();
    match (svc.as_str(), op) {
        // SessionService
        ("session", "authenticate") => rr!(decode_auth_request, authenticate, encode_session_info),
        ("session", "whoami") => rr!(decode_page, whoami, encode_session_info),
        ("session", "logout") => rr!(decode_page, logout, encode_ok),

        // LibraryService
        ("library", "list-libraries") => {
            rr!(decode_page, list_libraries, encode_libraries_response)
        }
        ("library", "list-albums") => {
            rr!(decode_browse_request, list_albums, encode_albums_response)
        }
        ("library", "list-artists") => {
            rr!(decode_browse_request, list_artists, encode_artists_response)
        }
        ("library", "get-album") => rr!(decode_album_request, get_album, encode_album_detail),
        ("library", "get-artist") => rr!(decode_artist_request, get_artist, encode_artist_detail),
        ("library", "search") => rr!(decode_search_request, search, encode_search_response),
        ("library", "list-playlists") => {
            rr!(
                decode_browse_request,
                list_playlists,
                encode_playlists_response
            )
        }
        ("library", "get-playlist") => {
            rr!(
                decode_playlist_request,
                get_playlist,
                encode_playlist_detail
            )
        }
        ("library", "get-cover-art") => {
            rr!(decode_cover_art_request, get_cover_art, encode_cover_art)
        }
        ("library", "get-audiobook-progress") => rr!(
            decode_audiobook_progress_request,
            get_audiobook_progress,
            encode_audiobook_progress_response
        ),
        ("library", "update-audiobook-progress") => rr!(
            decode_update_audiobook_progress_request,
            update_audiobook_progress,
            encode_audiobook_progress
        ),

        // PlayerService (request/response subset; subscribe is a channel op)
        ("player", "list-players") => {
            rr!(
                decode_list_players_request,
                list_players,
                encode_list_players_response
            )
        }
        ("player", "control") => rr!(decode_command_request, control, encode_player_state),
        ("player", "enable-share") => {
            rr!(
                decode_enable_share_request,
                enable_share,
                encode_share_result
            )
        }
        ("player", "disable-share") => {
            rr!(decode_disable_share_request, disable_share, encode_ok)
        }

        // NodeService (register is request/response; session is a channel op)
        ("node", "register") => {
            rr!(
                decode_register_node_request,
                register,
                encode_register_node_response
            )
        }

        // AdminService
        ("admin", "list-accounts") => {
            rr!(decode_page, list_accounts, encode_list_accounts_response)
        }
        ("admin", "set-role") => rr!(decode_set_role_request, set_role, encode_account),
        ("admin", "trust-domain") => {
            rr!(
                decode_trust_domain_request,
                trust_domain,
                encode_trusted_domains
            )
        }
        ("admin", "list-trusted-domains") => {
            rr!(decode_page, list_trusted_domains, encode_trusted_domains)
        }
        ("admin", "list-nodes") => rr!(decode_page, list_nodes, encode_list_nodes_response),
        ("admin", "rename-node") => rr!(decode_rename_node_request, rename_node, encode_node_info),
        ("admin", "rename-device") => {
            rr!(
                decode_rename_device_request,
                rename_device,
                encode_device_info
            )
        }
        ("admin", "set-device-access") => rr!(
            decode_set_device_access_request,
            set_device_access,
            encode_device_info
        ),
        ("admin", "list-groups") => {
            rr!(decode_page, list_groups, encode_list_groups_response)
        }
        ("admin", "create-group") => {
            rr!(decode_create_group_request, create_group, encode_group_info)
        }
        ("admin", "set-group-members") => rr!(
            decode_set_group_members_request,
            set_group_members,
            encode_group_info
        ),
        ("admin", "delete-group") => {
            rr!(decode_delete_group_request, delete_group, encode_ok)
        }
        ("admin", "list-satellite-tokens") => rr!(
            decode_page,
            list_satellite_tokens,
            encode_list_satellite_tokens_response
        ),
        ("admin", "create-node-token") => {
            rr!(
                decode_create_node_token_request,
                create_node_token,
                encode_node_token_result
            )
        }
        ("admin", "revoke-satellite-token") => rr!(
            decode_revoke_satellite_token_request,
            revoke_satellite_token,
            encode_ok
        ),
        ("admin", "import-track") => {
            rr!(
                decode_import_track_request,
                import_track,
                encode_import_result
            )
        }
        ("admin", "get-settings") => rr!(decode_page, get_settings, encode_settings),
        ("admin", "set-setting") => rr!(decode_set_setting_request, set_setting, encode_settings),
        ("admin", "resync-library") => {
            rr!(decode_page, resync_library, encode_library_resync_status)
        }
        ("admin", "get-resync-status") => {
            rr!(decode_page, get_resync_status, encode_library_resync_status)
        }

        // Channel operations require the streaming transport (not this request path).
        ("player", "subscribe") | ("media", "stream") | ("node", "session") => Err(ServiceError {
            code: 501,
            message: format!(
                "{service}.{op} is a streaming op; not available over the request path (pre-alpha)"
            ),
        }),

        _ => Err(ServiceError {
            code: 404,
            message: format!("unknown operation {service}.{op}"),
        }),
    }
}

// ============================================================================
// CSIL-Events envelope (verbose profile) — the real wire, matching the web UI's
// `web/themes/default/src/lib/csil.ts`. One binary WS frame per envelope:
// a canonical-CBOR map `{ event, payload: 24(bstr), service?, id? }`. Control-plane
// events are `$`-prefixed and carry no `service` (implied ordinal 0).
// ============================================================================

pub struct EventEnvelope {
    pub service: Option<String>,
    pub event: String,
    pub id: Option<u64>,
    pub payload: Vec<u8>,
}

pub fn decode_event_envelope(bytes: &[u8]) -> anyhow::Result<EventEnvelope> {
    let v: Value = ciborium::from_reader(bytes)?;
    let map = match &v {
        Value::Map(m) => m,
        _ => anyhow::bail!("envelope is not a CBOR map"),
    };
    let mut service = None;
    let mut event = None;
    let mut id = None;
    let mut payload = Vec::new();
    for (k, val) in map {
        let Value::Text(key) = k else { continue };
        match key.as_str() {
            "service" => {
                if let Value::Text(s) = val {
                    service = Some(s.clone());
                }
            }
            "event" => {
                if let Value::Text(s) = val {
                    event = Some(s.clone());
                }
            }
            "id" => {
                if let Value::Integer(i) = val {
                    id = u64::try_from(*i).ok();
                }
            }
            "payload" => {
                if let Value::Tag(24, inner) = val {
                    if let Value::Bytes(b) = inner.as_ref() {
                        payload = b.clone();
                    }
                }
            }
            _ => {}
        }
    }
    Ok(EventEnvelope {
        service,
        event: event.ok_or_else(|| anyhow::anyhow!("envelope has no `event`"))?,
        id,
        payload,
    })
}

pub fn encode_event_envelope(env: &EventEnvelope) -> Vec<u8> {
    // Canonical-ish key order (length then bytewise): id, event, payload, service.
    let mut entries: Vec<(Value, Value)> = Vec::new();
    if let Some(id) = env.id {
        entries.push((Value::Text("id".into()), Value::Integer(id.into())));
    }
    entries.push((Value::Text("event".into()), Value::Text(env.event.clone())));
    entries.push((
        Value::Text("payload".into()),
        Value::Tag(24, Box::new(Value::Bytes(env.payload.clone()))),
    ));
    if let Some(s) = &env.service {
        entries.push((Value::Text("service".into()), Value::Text(s.clone())));
    }
    let mut out = Vec::new();
    let _ = ciborium::into_writer(&Value::Map(entries), &mut out);
    out
}

fn cbor_map(pairs: Vec<(&str, Value)>) -> Vec<u8> {
    let entries = pairs
        .into_iter()
        .map(|(k, v)| (Value::Text(k.to_string()), v))
        .collect();
    let mut out = Vec::new();
    let _ = ciborium::into_writer(&Value::Map(entries), &mut out);
    out
}

/// Encode a `ServiceError` as the `{code, message}` map the client discriminates on.
fn encode_service_error(e: &ServiceError) -> Vec<u8> {
    cbor_map(vec![
        ("code", Value::Integer(e.code.into())),
        ("message", Value::Text(e.message.clone())),
    ])
}

/// Handle one inbound CSIL-Events frame. Returns the (possibly updated) connection identity
/// and an optional reply frame to send back.
/// The side effects a decoded frame asks the connection to perform, beyond sending the reply.
#[derive(Default)]
pub struct FrameEffects {
    /// A `player.Subscribe`: register this connection for live pushes of this player.
    pub subscribe: Option<String>,
    /// A successful `player.EnableShare`: this connection is now the device's output (§6).
    pub attach: Option<String>,
    /// A `node.Session`: register this connection for directives to this satellite player.
    pub node_session: Option<String>,
    /// A `media.Stream` open request: start sending MediaEvent frames for the requested track.
    pub media_open: Option<MediaOpen>,
}

/// Handle one inbound frame. Returns the (possibly updated) identity, an optional immediate
/// reply, and the connection-level side effects the caller must apply (subscribe / attach).
pub fn handle_events_frame(
    app: &App,
    ident: Identity,
    bytes: &[u8],
) -> (Identity, Option<Vec<u8>>, FrameEffects) {
    let env = match decode_event_envelope(bytes) {
        Ok(e) => e,
        Err(e) => {
            log::debug!("undecodable events frame: {e}");
            return (ident, None, FrameEffects::default());
        }
    };

    // Control plane ($-prefixed, no service).
    if let Some(stripped) = env.event.strip_prefix('$') {
        let (ident, reply) = handle_control(app, ident, stripped, &env.payload);
        return (ident, reply, FrameEffects::default());
    }

    let raw_service = env.service.clone().unwrap_or_default();
    let service = raw_service
        .strip_suffix("Service")
        .unwrap_or(&raw_service)
        .to_ascii_lowercase();
    let ctx = Ctx {
        identity: ident.clone(),
    };

    // Request/response: has a correlation id.
    if let Some(id) = env.id {
        let result = dispatch(app, &ctx, &service, &env.event, &env.payload);
        // A successful EnableShare makes this connection the device's output; capture the id
        // so the caller can register live presence for it.
        let attach = if service == "player" && env.event == "enable-share" {
            result
                .as_ref()
                .ok()
                .and_then(|p| decode_share_result(p).ok())
                .map(|r| r.player.id)
        } else {
            None
        };
        let payload = match result {
            Ok(p) => p,
            Err(se) => encode_service_error(&se),
        };
        let reply = encode_event_envelope(&EventEnvelope {
            service: None,
            event: env.event.clone(),
            id: Some(id),
            payload,
        });
        return (
            ident,
            Some(reply),
            FrameEffects {
                subscribe: None,
                attach,
                node_session: None,
                media_open: None,
            },
        );
    }

    // Channel event (no id): a subscribe registers for live pushes and gets an initial state.
    if service == "player" && env.event == "subscribe" {
        if let Some((reply, player_id)) = subscribe_snapshot(app, &ident, &env.payload) {
            return (
                ident,
                Some(reply),
                FrameEffects {
                    subscribe: Some(player_id),
                    attach: None,
                    node_session: None,
                    media_open: None,
                },
            );
        }
    }
    if service == "node" && env.event == "session" {
        if !matches!(ident, Identity::Node { .. }) {
            return (ident, None, FrameEffects::default());
        }
        if let Ok(report) = decode_node_report(&env.payload) {
            let player_id = report.player_id.clone();
            if !app.can_access_player(&ident, &player_id) {
                return (ident, None, FrameEffects::default());
            }
            let _ = app.record_node_report(report);
            return (
                ident,
                None,
                FrameEffects {
                    subscribe: None,
                    attach: None,
                    node_session: Some(player_id),
                    media_open: None,
                },
            );
        }
    }
    if service == "media" && env.event == "stream" {
        if !matches!(ident, Identity::Node { .. }) {
            return (ident, None, FrameEffects::default());
        }
        if let Ok(MediaControl::Variant0(open)) = decode_media_control(&env.payload) {
            return (
                ident,
                None,
                FrameEffects {
                    subscribe: None,
                    attach: None,
                    node_session: None,
                    media_open: Some(open),
                },
            );
        }
    }
    let _ = &ctx;
    (ident, None, FrameEffects::default())
}

/// Encode a `PlayerState` as a `player.subscribe` channel-push frame.
pub fn player_state_frame(state: &PlayerState) -> Vec<u8> {
    encode_event_envelope(&EventEnvelope {
        service: Some("player".to_string()),
        event: "subscribe".to_string(),
        id: None,
        payload: encode_player_state(state),
    })
}

fn handle_control(
    app: &App,
    mut ident: Identity,
    control: &str,
    payload: &[u8],
) -> (Identity, Option<Vec<u8>>) {
    match control {
        "hello" => {
            // Resolve auth token (if any) to an identity for the rest of the connection.
            if let Ok(Value::Map(map)) = ciborium::from_reader::<Value, _>(payload) {
                for (k, v) in &map {
                    if let (Value::Text(key), Value::Text(tok)) = (k, v) {
                        if key == "auth" {
                            let hash = crate::auth::sha256_hex(tok);
                            if let Ok(mut conn) = app.pool.get() {
                                if let Ok(Some(acct)) =
                                    crate::db::store::account_for_token(&mut conn, &hash)
                                {
                                    ident = Identity::User {
                                        account_id: acct.id,
                                        role: acct.role,
                                    };
                                }
                            }
                        } else if key == "node_token" {
                            let hash = crate::auth::sha256_hex(tok);
                            if let Ok(mut conn) = app.pool.get() {
                                let pending_key = format!("node_token:{hash}");
                                let configured =
                                    app.config.node_token.as_ref().is_some_and(|configured| {
                                        crate::auth::sha256_hex(configured) == hash
                                    });
                                if let Ok(Some(satellite)) =
                                    crate::db::store::satellite_for_hash(&mut conn, &hash)
                                {
                                    ident = Identity::Node {
                                        node_id: satellite.id,
                                    };
                                } else if configured
                                    || crate::db::store::get_setting(&mut conn, &pending_key)
                                        .ok()
                                        .flatten()
                                        .is_some()
                                {
                                    ident = Identity::Node {
                                        node_id: format!("pending:{hash}"),
                                    };
                                }
                            }
                        }
                    }
                }
            }
            let ack = encode_event_envelope(&EventEnvelope {
                service: None,
                event: "$hello-ack".to_string(),
                id: None,
                payload: cbor_map(vec![
                    ("version", Value::Integer(1.into())),
                    ("profile", Value::Text("verbose".into())),
                ]),
            });
            (ident, Some(ack))
        }
        "ping" => {
            let pong = encode_event_envelope(&EventEnvelope {
                service: None,
                event: "$pong".to_string(),
                id: None,
                payload: cbor_map(vec![("nonce", Value::Integer(0.into()))]),
            });
            (ident, Some(pong))
        }
        _ => (ident, None),
    }
}

/// On player.subscribe, return the initial state frame and the subscribed player id.
fn subscribe_snapshot(app: &App, identity: &Identity, payload: &[u8]) -> Option<(Vec<u8>, String)> {
    let req = decode_subscribe_request(payload).ok()?;
    if !app.can_access_player(identity, &req.player_id) {
        return None;
    }
    let mut conn = app.pool.get().ok()?;
    let state = app.load_player_state(&mut conn, &req.player_id).ok()?;
    Some((player_state_frame(&state), req.player_id))
}
