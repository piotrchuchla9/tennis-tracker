mod support;

use std::sync::Arc;
use support::FakeTransport;
use tracker_core::clock::SyncSession;
use tracker_core::link::{LinkMessage, PeerLink};

fn setup() -> (Arc<FakeTransport>, Arc<PeerLink>, SyncSession) {
    let transport = Arc::new(FakeTransport::new());
    let link = Arc::new(PeerLink::new(transport.clone()));
    let session = SyncSession::new(link.clone());
    (transport, link, session)
}

fn ping_count(transport: &FakeTransport) -> usize {
    transport
        .sent_messages()
        .iter()
        .filter(|m| matches!(m, LinkMessage::Ping { .. }))
        .count()
}

/// Odpowiada na wszystkie wyslane pingi tak, jak zrobilby to peer
/// o zadanym offsecie i symetrycznej sciezce.
fn reply_to_pings(transport: &FakeTransport, session: &mut SyncSession, offset: f64) {
    let one_way = 0.005;
    for message in transport.sent_messages() {
        if let LinkMessage::Ping { id, t1 } = message {
            let t2 = t1 + one_way + offset;
            let t3 = t2 + 0.001;
            let t4 = t3 - offset + one_way;
            session.handle(&LinkMessage::Pong { id, t1, t2, t3 }, t4);
        }
    }
}

#[test]
fn burst_sends_fifty_pings_on_start() {
    let (transport, _link, mut session) = setup();
    session.start(1000.0);
    assert_eq!(ping_count(&transport), 50);
}

#[test]
fn produces_fit_after_burst_is_answered() {
    let (transport, _link, mut session) = setup();
    session.start(1000.0);
    reply_to_pings(&transport, &mut session, 0.4);

    let fit = session.current_fit().expect("model powinien powstac");
    assert!(
        (fit.offset_seconds - 0.4).abs() < 0.001,
        "offset = {}",
        fit.offset_seconds
    );
}

#[test]
fn steady_state_sends_one_ping_per_second() {
    let (transport, _link, mut session) = setup();
    session.start(1000.0);
    transport.clear_sent();

    session.tick(1000.5);
    assert_eq!(transport.sent_count(), 0);

    session.tick(1001.0);
    assert_eq!(transport.sent_count(), 1);

    session.tick(1002.0);
    assert_eq!(transport.sent_count(), 2);
}

#[test]
fn unmatched_pong_is_ignored() {
    let (_transport, _link, mut session) = setup();
    session.start(1000.0);
    session.handle(
        &LinkMessage::Pong {
            id: 99_999,
            t1: 1.0,
            t2: 2.0,
            t3: 3.0,
        },
        4.0,
    );
    assert_eq!(session.samples().len(), 0);
}

#[test]
fn ping_is_answered_with_pong() {
    let (transport, _link, mut session) = setup();
    session.handle(&LinkMessage::Ping { id: 5, t1: 700.0 }, 700.31);

    let last = transport
        .sent_messages()
        .pop()
        .expect("oczekiwano odpowiedzi");
    match last {
        LinkMessage::Pong { id, t1, t2, t3 } => {
            assert_eq!(id, 5);
            assert!((t1 - 700.0).abs() < 1e-9);
            assert!((t2 - 700.31).abs() < 1e-9);
            assert!(t3 >= t2);
        }
        other => panic!("oczekiwano ponga, otrzymano {other:?}"),
    }
}

#[test]
fn gap_is_recorded_when_link_drops() {
    let (_transport, _link, mut session) = setup();
    session.start(1000.0);
    session.link_did_change(false, 1100.0);
    session.link_did_change(true, 1160.0);

    assert_eq!(session.gaps().len(), 1);
    let gap = &session.gaps()[0];
    assert!((gap.start_host_time - 1100.0).abs() < 1e-9);
    assert_eq!(gap.end_host_time, Some(1160.0));
    assert_eq!(gap.reason, "link-lost");
}

#[test]
fn gap_stays_open_while_disconnected() {
    let (_transport, _link, mut session) = setup();
    session.start(1000.0);
    session.link_did_change(false, 1100.0);

    assert_eq!(session.gaps().len(), 1);
    assert_eq!(session.gaps()[0].end_host_time, None);
}

#[test]
fn no_pings_are_sent_while_disconnected() {
    let (transport, _link, mut session) = setup();
    session.start(1000.0);
    session.link_did_change(false, 1000.0);
    transport.clear_sent();

    session.tick(1005.0);

    assert_eq!(transport.sent_count(), 0);
}

#[test]
fn snapshot_carries_model_samples_and_gaps() {
    let (transport, _link, mut session) = setup();
    session.start(1000.0);
    reply_to_pings(&transport, &mut session, 0.4);
    session.link_did_change(false, 1100.0);
    session.link_did_change(true, 1120.0);

    let snapshot = session.snapshot();
    assert!((snapshot.model.offset_seconds - 0.4).abs() < 0.001);
    assert_eq!(snapshot.samples.len(), 50);
    assert_eq!(snapshot.gaps.len(), 1);
}

#[test]
fn snapshot_without_fit_reports_infinite_residual() {
    let (_transport, _link, session) = setup();
    let snapshot = session.snapshot();
    assert!(snapshot.model.residual_std_ms.is_infinite());
    assert_eq!(snapshot.model.sample_count, 0);
}
