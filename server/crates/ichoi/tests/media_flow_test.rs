mod common;
use ichoi::{
    handlers::Identity,
    transport::{self, EventEnvelope},
};
use libichoi::{
    csil::types::{MediaControl, MediaPause, MediaResume},
    csil_channel::encode_media_control,
};

#[test]
fn only_nodes_can_pause_and_resume_their_connection_media_producers() {
    let (app, _pool) = common::test_app();
    for (control, paused) in [
        (
            MediaControl::Variant2(MediaPause {
                kind: "pause".into(),
                stream_id: "stream".into(),
            }),
            true,
        ),
        (
            MediaControl::Variant3(MediaResume {
                kind: "resume".into(),
                stream_id: "stream".into(),
            }),
            false,
        ),
    ] {
        let frame = transport::encode_event_envelope(&EventEnvelope {
            service: Some("media".into()),
            event: "stream".into(),
            id: None,
            payload: encode_media_control(&control),
        });
        let (_, _, effects) = transport::handle_events_frame(
            &app,
            Identity::Node {
                node_id: "speaker".into(),
            },
            false,
            &frame,
        );
        assert_eq!(effects.media_pause, Some(("stream".into(), paused)));
        let (_, _, effects) =
            transport::handle_events_frame(&app, Identity::Anonymous, true, &frame);
        assert!(effects.media_pause.is_none());
    }
}
