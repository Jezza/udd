use super::*;

#[test]
fn roundtrip_connect() {
    let connect = Connect {
        client_id: "test-client".into(),
        keep_alive: 120,
        clean_session: true,
        username: Some("user".into()),
        password: Some(b"pass".to_vec()),
    };

    let frame = UdpFrame::new(1, connect.clone());
    let encoded = frame.encode();
    let decoded = UdpFrame::decode(&encoded).unwrap();

    assert_eq!(decoded.msg_id, 1);
    if let Packet::Connect(c) = decoded.packet {
        assert_eq!(c, connect);
    } else {
        panic!("wrong packet type");
    }
}

#[test]
fn roundtrip_publish() {
    let publish = Publish::new("sensor/temp", b"25.5")
        .with_qos(QoS::AtLeastOnce)
        .with_retain(true);

    let frame = UdpFrame::new(42, publish.clone());
    let encoded = frame.encode();
    let decoded = UdpFrame::decode(&encoded).unwrap();

    assert_eq!(decoded.msg_id, 42);
    if let Packet::Publish(p) = decoded.packet {
        assert_eq!(p, publish);
    } else {
        panic!("wrong packet type");
    }
}

#[test]
fn roundtrip_subscribe() {
    let subscribe = Subscribe::new(vec![
        SubscribeFilter::new("home/+/temp", QoS::AtLeastOnce),
        SubscribeFilter::new("office/#", QoS::AtMostOnce),
    ]);

    let frame = UdpFrame::new(100, subscribe.clone());
    let encoded = frame.encode();
    let decoded = UdpFrame::decode(&encoded).unwrap();

    if let Packet::Subscribe(s) = decoded.packet {
        assert_eq!(s, subscribe);
    } else {
        panic!("wrong packet type");
    }
}

#[test]
fn invalid_message_type() {
    let buf = [4, 0xFF, 0, 0]; // Invalid type 0xFF
    let result = UdpFrame::decode(&buf);
    assert!(matches!(result, Err(DecodeError::InvalidMessageType(0xFF))));
}

#[test]
fn buffer_too_short() {
    let buf = [10, 0x01, 0, 0]; // Claims length 10, only 4 bytes
    let result = UdpFrame::decode(&buf);
    assert!(matches!(result, Err(DecodeError::BufferTooShort { .. })));
}
