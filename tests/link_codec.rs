use tracker_core::clock::ClockSample;
use tracker_core::link::{decode, encode, LinkEnvelope, LinkMessage};
use tracker_core::session::{CaptureProfile, SessionRole};

fn round_trip(message: LinkMessage) {
    let envelope = LinkEnvelope { sequence: 42, message };
    let bytes = encode(&envelope).expect("kodowanie powinno sie udac");
    let decoded = decode(&bytes).expect("dekodowanie powinno sie udac");
    assert_eq!(decoded, envelope);
}

#[test]
fn round_trips_every_message_case() {
    round_trip(LinkMessage::Hello {
        device_model: "iPhone17,2".into(),
        app_version: "0.1.0".into(),
        preferred_role: SessionRole::Master,
    });
    round_trip(LinkMessage::RoleAssigned { role: SessionRole::Slave });
    round_trip(LinkMessage::Ping { id: 7, t1: 1000.5 });
    round_trip(LinkMessage::Pong { id: 7, t1: 1000.5, t2: 1000.75, t3: 1000.751 });
    round_trip(LinkMessage::StartRecording {
        session_id: "s-1".into(),
        profile: CaptureProfile::P1080p120,
        host_time: 12.5,
    });
    round_trip(LinkMessage::StopRecording { host_time: 900.0 });
    round_trip(LinkMessage::Heartbeat { host_time: 33.0 });
    round_trip(LinkMessage::SyncLog {
        samples: vec![ClockSample::new(1.0, 2.0, 3.0, 4.0)],
    });
}

#[test]
fn decoding_garbage_fails() {
    assert!(decode(&[0x00, 0x01, 0x02]).is_err());
}

#[test]
fn capture_profile_geometry() {
    assert_eq!(CaptureProfile::P1080p120.width(), 1920);
    assert_eq!(CaptureProfile::P1080p120.height(), 1080);
    assert_eq!(CaptureProfile::P1080p120.fps(), 120);

    assert_eq!(CaptureProfile::P4k60.width(), 3840);
    assert_eq!(CaptureProfile::P4k60.height(), 2160);
    assert_eq!(CaptureProfile::P4k60.fps(), 60);

    assert_eq!(CaptureProfile::P1080p60.width(), 1920);
    assert_eq!(CaptureProfile::P1080p60.fps(), 60);

    assert_eq!(CaptureProfile::P1080p30.width(), 1920);
    assert_eq!(CaptureProfile::P1080p30.fps(), 30);
}

#[test]
fn profile_strings_match_manifest_vocabulary() {
    assert_eq!(CaptureProfile::P1080p120.as_str(), "1080p120");
    assert_eq!(CaptureProfile::P4k60.as_str(), "4K60");
    assert_eq!(CaptureProfile::P1080p60.as_str(), "1080p60");
    assert_eq!(CaptureProfile::P1080p30.as_str(), "1080p30");

    assert_eq!(CaptureProfile::from_str("1080p120"), Some(CaptureProfile::P1080p120));
    assert_eq!(CaptureProfile::from_str("4K60"), Some(CaptureProfile::P4k60));
    assert_eq!(CaptureProfile::from_str("bzdura"), None);
}

#[test]
fn role_has_an_opposite() {
    assert_eq!(SessionRole::Master.opposite(), SessionRole::Slave);
    assert_eq!(SessionRole::Slave.opposite(), SessionRole::Master);
}
