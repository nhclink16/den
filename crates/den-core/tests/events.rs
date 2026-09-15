use den_core::Event;

#[test]
fn unknown_event_types_deserialize() {
    let event = serde_json::from_str::<Event>(
        r#"{"type":"future_notification","payload":{"nested":[1,true,null]}}"#,
    )
    .expect("unknown event types must not break older clients");
    assert_eq!(event, Event::Unknown);
    assert!(
        serde_json::to_string(&event).is_err(),
        "fallback is receive-only"
    );
}

#[test]
fn malformed_known_events_still_fail() {
    for payload in [
        r#"{"type":"presence","user_id":"alice"}"#,
        r#"{"type":"presence","user_id":"alice","online":"yes"}"#,
        r#"{"user_id":"alice","online":true}"#,
        r#"{"type":42}"#,
    ] {
        assert!(serde_json::from_str::<Event>(payload).is_err(), "{payload}");
    }
}
