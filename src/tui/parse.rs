use mqtt::{
    ConnAck, Connect, ConnectReturnCode, Disconnect, Packet, Ping, Pong, PubAck, Publish,
    QoS, SubAck, SubAckReturnCode, Subscribe, SubscribeFilter, UdpFrame,
};
use crate::tui::next_msg_id;

/// Parse MQTT command syntax into a UdpFrame
pub fn parse_mqtt_command(input: &str) -> mqtt::Result<UdpFrame, String> {
    let input = input.trim();
    let (cmd, rest) = input.split_once(' ').unwrap_or((input, ""));
    let rest = rest.trim();

    let packet: Packet = match cmd.to_lowercase().as_str() {
        "connect" => {
            // connect <client_id> [keepalive=N] [user=X] [pass=X]
            let mut parts = rest.split_whitespace();

            let client_id = parts.next().unwrap_or("id1");
            let mut conn = Connect::new(client_id);

            for part in parts {
                if let Some((k, v)) = part.split_once('=') {
                    match k {
                        "keepalive" | "ka" => {
                            conn.keep_alive = v.parse().map_err(|_| "invalid keepalive")?;
                        }
                        "user" => conn.username = Some(v.to_string()),
                        "pass" => conn.password = Some(v.as_bytes().to_vec()),
                        "clean" => conn.clean_session = v == "true" || v == "1",
                        _ => return Err(format!("unknown option: {}", k)),
                    }
                }
            }
            conn.into()
        }

        "pub" | "publish" => {
            // pub <topic> <payload> [qos=0|1|2] [retain]
            let (topic, remainder) = rest
                .split_once(' ')
                .ok_or("pub|publish <topic> <payload> [qos=0|1|2] [retain]")?;
            let mut pub_pkt = Publish::new(topic, "");
            let mut payload_parts = vec![];

            for part in remainder.split_whitespace() {
                if let Some((k, v)) = part.split_once('=') {
                    match k {
                        "qos" => {
                            pub_pkt = pub_pkt.with_qos(match v {
                                "0" => QoS::AtMostOnce,
                                "1" => QoS::AtLeastOnce,
                                "2" => QoS::ExactlyOnce,
                                _ => return Err("qos must be 0, 1, or 2".into()),
                            });
                        }
                        _ => payload_parts.push(part),
                    }
                } else if part == "retain" {
                    pub_pkt = pub_pkt.with_retain(true);
                } else {
                    payload_parts.push(part);
                }
            }

            pub_pkt.payload = payload_parts.join(" ").into_bytes();
            pub_pkt.into()
        }

        "sub" | "subscribe" => {
            // sub <topic> [qos=0|1|2]
            // sub <topic1>,<topic2> [qos=0|1|2]
            let mut qos = QoS::AtMostOnce;
            let mut topics = vec![];

            for part in rest.split_whitespace() {
                if let Some((k, v)) = part.split_once('=') {
                    if k == "qos" {
                        qos = match v {
                            "0" => QoS::AtMostOnce,
                            "1" => QoS::AtLeastOnce,
                            "2" => QoS::ExactlyOnce,
                            _ => return Err("qos must be 0, 1, or 2".into()),
                        };
                    }
                } else {
                    topics.extend(part.split(',').map(|s| s.to_string()));
                }
            }

            if topics.is_empty() {
                return Err("subscribe requires at least one topic".into());
            }

            let filters = topics
                .into_iter()
                .map(|t| SubscribeFilter::new(t, qos))
                .collect();
            Subscribe::new(filters).into()
        }

        "ping" => Ping.into(),
        "disconnect" | "disc" => Disconnect.into(),
        "puback" => PubAck.into(),

        "connack" => {
            // connack [accepted|rejected] [session=true|false]
            let mut code = ConnectReturnCode::Accepted;
            let mut session = false;

            for part in rest.split_whitespace() {
                match part {
                    "accepted" => code = ConnectReturnCode::Accepted,
                    "rejected" | "unauthorized" => code = ConnectReturnCode::NotAuthorized,
                    "unavailable" => code = ConnectReturnCode::ServerUnavailable,
                    _ if part.starts_with("session=") => {
                        session = part.ends_with("true") || part.ends_with("1");
                    }
                    _ => {}
                }
            }
            ConnAck {
                session_present: session,
                return_code: code,
            }
            .into()
        }

        "suback" => {
            // suback 0 1 2 or suback fail
            let codes: mqtt::Result<Vec<_>, _> = rest
                .split_whitespace()
                .map(|s| match s {
                    "0" => Ok(SubAckReturnCode::SuccessQoS0),
                    "1" => Ok(SubAckReturnCode::SuccessQoS1),
                    "2" => Ok(SubAckReturnCode::SuccessQoS2),
                    "fail" | "failure" => Ok(SubAckReturnCode::Failure),
                    _ => Err(format!("invalid suback code: {}", s)),
                })
                .collect();
            SubAck::new(codes?).into()
        }

        "pingresp" | "pong" => Pong.into(),

        _ => return Err(format!("unknown command: {}", cmd)),
    };

    Ok(UdpFrame::new(next_msg_id(), packet))
}
