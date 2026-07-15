//! The real CSIL-Events wire path (verbose envelope) exercised end-to-end: build the exact
//! frames the browser sends, feed them through `handle_events_frame`, and decode the replies.

mod common;

use ciborium::value::Value;
use ichoi::handlers::Identity;
use ichoi::transport::{
    decode_event_envelope, encode_event_envelope, handle_events_frame, player_state_frame,
    EventEnvelope,
};
use libichoi::csil::codec::{decode_albums_response, decode_player_state, encode_browse_request};
use libichoi::csil::types::{BrowseRequest, PlayerState, PlayerStatus, QueueItem};

use common::DataMap;

fn tag24(payload: Vec<u8>) -> Value {
    Value::Tag(24, Box::new(Value::Bytes(payload)))
}

/// Encode the `$hello` control frame exactly as the web UI does.
fn hello_frame(auth: Option<&str>) -> Vec<u8> {
    let mut entries = vec![
        (
            Value::Text("versions".into()),
            Value::Array(vec![Value::Integer(1.into())]),
        ),
        (
            Value::Text("profiles".into()),
            Value::Array(vec![Value::Text("verbose".into())]),
        ),
    ];
    if let Some(a) = auth {
        entries.push((Value::Text("auth".into()), Value::Text(a.into())));
    }
    let mut payload = Vec::new();
    ciborium::into_writer(&Value::Map(entries), &mut payload).unwrap();
    encode_event_envelope(&EventEnvelope {
        service: None,
        event: "$hello".to_string(),
        id: None,
        payload,
    })
}

#[test]
fn hello_gets_ack() {
    let (app, _pool) = common::test_app();
    let (_ident, reply, _fx) = handle_events_frame(&app, Identity::Anonymous, &hello_frame(None));
    let env = decode_event_envelope(&reply.expect("ack frame")).unwrap();
    assert_eq!(env.event, "$hello-ack");
}

#[test]
fn request_reply_round_trips_over_the_wire() {
    let (app, pool) = common::test_app();
    {
        let mut conn = pool.get().unwrap();
        common::create_artist(&mut conn, &DataMap::new());
        common::create_album(&mut conn, &DataMap::new());
        common::create_track(&mut conn, &DataMap::new());
    }

    // Build the exact frame the browser sends for library.list-albums.
    let req_payload = encode_browse_request(&BrowseRequest {
        library: None,
        offset: None,
        limit: None,
    });
    let frame = encode_event_envelope(&EventEnvelope {
        service: Some("library".to_string()),
        event: "list-albums".to_string(),
        id: Some(7),
        payload: req_payload,
    });

    let (_ident, reply, _fx) = handle_events_frame(&app, Identity::Anonymous, &frame);
    let env = decode_event_envelope(&reply.expect("reply frame")).unwrap();

    assert_eq!(env.id, Some(7), "reply correlates by id");
    let resp = decode_albums_response(&env.payload).expect("decode AlbumsResponse from payload");
    assert_eq!(resp.total, 1);
    assert_eq!(resp.albums[0].title, "Test Album");
}

#[test]
fn error_rides_as_service_error_map() {
    let (app, _pool) = common::test_app();
    // Unknown album → 404 ServiceError, carried in the payload slot as {code, message}.
    let req = encode_browse_request(&BrowseRequest {
        library: None,
        offset: None,
        limit: None,
    });
    let frame = encode_event_envelope(&EventEnvelope {
        service: Some("library".to_string()),
        event: "get-album".to_string(), // wrong request type → decode error → 400
        id: Some(9),
        payload: req,
    });
    let (_ident, reply, _fx) = handle_events_frame(&app, Identity::Anonymous, &frame);
    let env = decode_event_envelope(&reply.expect("reply")).unwrap();
    assert_eq!(env.id, Some(9));
    // Decode the payload as a generic CBOR map and assert it's a {code, message} error.
    let v: Value = ciborium::from_reader(env.payload.as_slice()).unwrap();
    let Value::Map(m) = v else {
        panic!("payload not a map")
    };
    let keys: Vec<String> = m
        .iter()
        .filter_map(|(k, _)| match k {
            Value::Text(t) => Some(t.clone()),
            _ => None,
        })
        .collect();
    assert!(keys.contains(&"code".to_string()) && keys.contains(&"message".to_string()));
}

#[test]
fn hello_auth_token_resolves_identity() {
    let (app, pool) = common::test_app();
    // Seed an account + session token.
    {
        let mut conn = pool.get().unwrap();
        ichoi::db::store::upsert_account(
            &mut conn,
            &ichoi::db::models::Account {
                id: "u@example.com".into(),
                handle: "u".into(),
                display_name: None,
                role: "admin".into(),
                created_at: chrono::Utc::now().to_rfc3339(),
            },
        )
        .unwrap();
        let hash = ichoi::auth::sha256_hex("secret-token");
        ichoi::db::store::create_session(&mut conn, &hash, "u@example.com", "2099-01-01T00:00:00Z")
            .unwrap();
    }

    let (ident, _reply, _fx) = handle_events_frame(
        &app,
        Identity::Anonymous,
        &hello_frame(Some("secret-token")),
    );
    match ident {
        Identity::User { account_id, role } => {
            assert_eq!(account_id, "u@example.com");
            assert_eq!(role, "admin");
        }
        _ => panic!("hello with a valid token should resolve a user identity"),
    }
}

#[test]
fn player_state_push_frame_matches_the_subscribe_channel() {
    // The server pushes live state as a channel event the browser listens for under
    // PlayerService.subscribe. Lock the wire shape: service "player", event "subscribe",
    // and a payload the same codec the client uses can decode back to a PlayerState.
    let state = PlayerState {
        player_id: "share:guest:TodPhone".to_string(),
        status: PlayerStatus::Playing,
        current_index: Some(1),
        position_ms: Some(4200),
        volume: 100,
        queue: vec![
            QueueItem {
                track_id: "t1".into(),
                title: Some("One".into()),
                artist: None,
                duration_ms: Some(1000),
            },
            QueueItem {
                track_id: "t2".into(),
                title: Some("Two".into()),
                artist: None,
                duration_ms: Some(2000),
            },
        ],
    };

    let env = decode_event_envelope(&player_state_frame(&state)).unwrap();
    assert_eq!(env.service.as_deref(), Some("player"));
    assert_eq!(env.event, "subscribe");
    assert_eq!(
        env.id, None,
        "pushes are fire-and-forget, not correlated replies"
    );

    let decoded = decode_player_state(&env.payload).expect("payload decodes as PlayerState");
    assert_eq!(decoded.player_id, "share:guest:TodPhone");
    assert_eq!(decoded.current_index, Some(1));
    assert_eq!(decoded.queue.len(), 2);
    assert_eq!(decoded.queue[1].track_id, "t2");
}

// Silence unused-import warnings for the tag24 helper on some cfgs.
#[allow(dead_code)]
fn _use_tag24() {
    let _ = tag24(vec![]);
}
