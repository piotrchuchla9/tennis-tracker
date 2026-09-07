use tracker_core::remote::{command_to_event, MarkTag, RemoteCommand, RemoteFeedback};

#[test]
fn mark_command_becomes_a_manifest_event() {
    let event = command_to_event(&RemoteCommand::MarkMoment { tag: MarkTag::Changeover }, 1234.5)
        .expect("oczekiwano zdarzenia");

    assert_eq!(event.event_type, "mark");
    assert_eq!(event.tag.as_deref(), Some("changeover"));
    assert!((event.host_time - 1234.5).abs() < 1e-9);
}

#[test]
fn transport_commands_produce_no_event() {
    assert!(command_to_event(&RemoteCommand::StartRecording, 1.0).is_none());
    assert!(command_to_event(&RemoteCommand::StopRecording, 1.0).is_none());
}

#[test]
fn mark_tags_match_manifest_vocabulary() {
    assert_eq!(MarkTag::Interesting.as_str(), "interesting");
    assert_eq!(MarkTag::Changeover.as_str(), "changeover");
    assert_eq!(MarkTag::StrayBall.as_str(), "stray-ball");
    assert_eq!(MarkTag::BadRally.as_str(), "bad-rally");
}

#[test]
fn feedback_variants_carry_their_payload() {
    match (RemoteFeedback::SyncQuality { residual_ms: 1.2 }) {
        RemoteFeedback::SyncQuality { residual_ms } => assert!((residual_ms - 1.2).abs() < 1e-9),
        other => panic!("nieoczekiwany wariant: {other:?}"),
    }
    match (RemoteFeedback::RecordingState { recording: true, elapsed_seconds: 42.0 }) {
        RemoteFeedback::RecordingState { recording, elapsed_seconds } => {
            assert!(recording);
            assert!((elapsed_seconds - 42.0).abs() < 1e-9);
        }
        other => panic!("nieoczekiwany wariant: {other:?}"),
    }
}
