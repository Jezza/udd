use crate::InputMode;
use mqtt::{Packet, UdpFrame};
use std::borrow::Cow;

/// Format payload for display
pub fn format(data: &[u8]) -> Cow<'_, str> {
    if let Some(pretty) = format_mqtt_frame(data) {
        return Cow::Owned(pretty);
    }

    if let Some(s) = str::from_utf8(data).ok() {
        return if s.len() > 50 {
            Cow::Owned(format!("{}...", &s[..47]))
        } else {
            Cow::Borrowed(s)
        };
    }

    let hex: String = data
        .iter()
        .take(24)
        .map(|b| format!("{:02x} ", b))
        .collect();
    let t = if data.len() > 24 {
        format!("{}...", hex.trim())
    } else {
        hex.trim().to_string()
    };
    Cow::Owned(t)
}

/// Format payload for display, honoring the selected input mode.
pub fn format_for_mode(mode: InputMode, data: &[u8]) -> Cow<'_, str> {
    match mode {
        InputMode::Hex => Cow::Owned(format_hex(data)),
        InputMode::Mqtt => Cow::Owned(format_mqtt_frame(data).unwrap_or_else(|| format_hex(data))),
        InputMode::Text => format_text(data).unwrap_or_else(|| Cow::Owned(format_hex(data))),
        InputMode::Auto => format(data),
    }
}

fn format_hex(data: &[u8]) -> String {
    let hex: String = data
        .iter()
        .take(24)
        .map(|b| format!("{:02x} ", b))
        .collect();
    if data.len() > 24 {
        format!("{}...", hex.trim())
    } else {
        hex.trim().to_string()
    }
}

fn format_text(data: &[u8]) -> Option<Cow<'_, str>> {
    str::from_utf8(data).ok().map(|s| {
        if s.len() > 50 {
            Cow::Owned(format!("{}...", &s[..47]))
        } else {
            Cow::Borrowed(s)
        }
    })
}

/// Decode and format MQTT frame for display
fn format_mqtt_frame(data: &[u8]) -> Option<String> {
    let frame = UdpFrame::decode(data).ok()?;

    let pkt_str = match &frame.packet {
        Packet::Connect(c) => {
            format!("CONNECT client={} ka={}", c.client_id, c.keep_alive)
        }
        Packet::ConnAck(c) => {
            format!("CONNACK {:?} session={}", c.return_code, c.session_present)
        }
        Packet::Publish(p) => {
            let payload_preview = String::from_utf8_lossy(&p.payload);
            let preview = if payload_preview.len() > 30 {
                format!("{}...", &payload_preview[..27])
            } else {
                payload_preview.into_owned()
            };
            format!("PUBLISH {} qos={:?} \"{}\"", p.topic, p.qos, preview)
        }
        Packet::PubAck(_) => "PUBACK".into(),
        Packet::Subscribe(s) => {
            let topics: Vec<_> = s.filters.iter().map(|f| f.topic.as_str()).collect();
            format!("SUBSCRIBE [{}]", topics.join(", "))
        }
        Packet::SubAck(s) => format!("SUBACK {:?}", s.return_codes),
        Packet::Ping(_) => "PING".into(),
        Packet::Pong(_) => "PONG".into(),
        Packet::Disconnect(_) => "DISCONNECT".into(),
    };

    Some(format!("#{} {}", frame.msg_id, pkt_str))
}
