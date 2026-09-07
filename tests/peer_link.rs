mod support;

use std::sync::Arc;
use support::FakeTransport;
use tracker_core::link::{encode, Delivery, LinkEnvelope, LinkMessage, PeerLink};

fn setup() -> (Arc<FakeTransport>, PeerLink) {
    let transport = Arc::new(FakeTransport::new());
    let link = PeerLink::new(transport.clone());
    (transport, link)
}

fn heartbeat(host_time: f64) -> LinkMessage {
    LinkMessage::Heartbeat { host_time }
}

fn packet(sequence: u64, host_time: f64) -> Vec<u8> {
    encode(&LinkEnvelope {
        sequence,
        message: heartbeat(host_time),
    })
    .unwrap()
}

#[test]
fn assigns_increasing_sequence_numbers() {
    let (transport, link) = setup();
    link.send(heartbeat(1.0)).unwrap();
    link.send(heartbeat(2.0)).unwrap();
    link.send(heartbeat(3.0)).unwrap();

    let sequences: Vec<u64> = transport
        .sent_envelopes()
        .iter()
        .map(|e| e.sequence)
        .collect();
    assert_eq!(sequences, vec![0, 1, 2]);
}

#[test]
fn delivers_received_message() {
    let (_transport, link) = setup();
    assert_eq!(link.receive(&packet(0, 5.0)), Some(heartbeat(5.0)));
}

#[test]
fn duplicate_sequence_is_dropped() {
    let (_transport, link) = setup();
    let data = packet(9, 5.0);
    assert!(link.receive(&data).is_some());
    assert!(link.receive(&data).is_none());
}

#[test]
fn out_of_order_messages_are_all_delivered() {
    let (_transport, link) = setup();
    assert_eq!(link.receive(&packet(2, 2.0)), Some(heartbeat(2.0)));
    assert_eq!(link.receive(&packet(1, 1.0)), Some(heartbeat(1.0)));
}

#[test]
fn malformed_packet_is_ignored_and_counted() {
    let (_transport, link) = setup();
    assert!(link.receive(&[0xFF, 0xFE]).is_none());
    assert_eq!(link.malformed_packet_count(), 1);
}

#[test]
fn send_while_disconnected_fails() {
    let (_transport, link) = setup();
    link.set_connected(false);
    assert!(link.send(heartbeat(1.0)).is_err());
}

#[test]
fn transport_failure_is_reported() {
    let (transport, link) = setup();
    transport.set_fail_send(true);
    assert!(link.send(heartbeat(1.0)).is_err());
}

#[test]
fn connection_state_is_tracked() {
    let (_transport, link) = setup();
    assert!(link.is_connected());
    link.set_connected(false);
    assert!(!link.is_connected());
    link.set_connected(true);
    assert!(link.is_connected());
}

/// Pingi po niezawodnym kanale zepsulyby pomiar opoznienia,
/// wiec trasowanie trybu jest czescia kontraktu warstwy lacza.
#[test]
fn pings_go_unreliable_and_commands_go_reliable() {
    let (transport, link) = setup();
    link.send(LinkMessage::Ping { id: 1, t1: 10.0 }).unwrap();
    link.send(LinkMessage::Pong {
        id: 1,
        t1: 10.0,
        t2: 10.1,
        t3: 10.2,
    })
    .unwrap();
    link.send(heartbeat(11.0)).unwrap();
    link.send(LinkMessage::StopRecording { host_time: 12.0 })
        .unwrap();

    assert_eq!(
        transport.sent_deliveries(),
        vec![
            Delivery::Unreliable,
            Delivery::Unreliable,
            Delivery::Reliable,
            Delivery::Reliable
        ]
    );
}

#[test]
fn sequence_memory_is_bounded() {
    let (_transport, link) = setup();
    for sequence in 0..5000u64 {
        assert!(link.receive(&packet(sequence, sequence as f64)).is_some());
    }
    assert!(link.remembered_sequence_count() <= 1024);
}
