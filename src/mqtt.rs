use std::fmt;

mod impls;

#[cfg(test)]
mod tests;

pub trait Encode {
    fn encode(&self, buf: &mut Vec<u8>);

    fn encoded_len(&self) -> usize;

    fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.encoded_len());
        self.encode(&mut buf);
        buf
    }
}

pub trait Decode: Sized {
    fn decode(buf: &[u8]) -> Result<Self>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    BufferTooShort { expected: usize, actual: usize },
    InvalidMessageType(u8),
    InvalidQoS(u8),
    InvalidReturnCode(u8),
    InvalidUtf8,
    PayloadTooLarge,
    MalformedPacket(&'static str),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooShort { expected, actual } => {
                write!(f, "buffer too short: expected {expected}, got {actual}")
            }
            Self::InvalidMessageType(v) => write!(f, "invalid message type: 0x{v:02X}"),
            Self::InvalidQoS(v) => write!(f, "invalid QoS: {v}"),
            Self::InvalidReturnCode(v) => write!(f, "invalid return code: 0x{v:02X}"),
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 string"),
            Self::PayloadTooLarge => write!(f, "payload exceeds maximum size"),
            Self::MalformedPacket(msg) => write!(f, "malformed packet: {msg}"),
        }
    }
}

impl std::error::Error for DecodeError {}

pub type Result<T, E = DecodeError> = std::result::Result<T, E>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MessageType {
    Connect = 0x01,
    ConnAck = 0x02,
    Publish = 0x03,
    PubAck = 0x04,
    Subscribe = 0x05,
    SubAck = 0x06,
    PingReq = 0x07,
    PingResp = 0x08,
    Disconnect = 0x09,
}

impl TryFrom<u8> for MessageType {
    type Error = DecodeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(Self::Connect),
            0x02 => Ok(Self::ConnAck),
            0x03 => Ok(Self::Publish),
            0x04 => Ok(Self::PubAck),
            0x05 => Ok(Self::Subscribe),
            0x06 => Ok(Self::SubAck),
            0x07 => Ok(Self::PingReq),
            0x08 => Ok(Self::PingResp),
            0x09 => Ok(Self::Disconnect),
            v => Err(DecodeError::InvalidMessageType(v)),
        }
    }
}

impl From<MessageType> for u8 {
    fn from(value: MessageType) -> Self {
        value as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum QoS {
    #[default]
    AtMostOnce = 0,
    AtLeastOnce = 1,
    ExactlyOnce = 2,
}

impl TryFrom<u8> for QoS {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::AtMostOnce),
            1 => Ok(Self::AtLeastOnce),
            2 => Ok(Self::ExactlyOnce),
            v => Err(v),
        }
    }
}

impl From<QoS> for u8 {
    fn from(value: QoS) -> Self {
        value as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ConnectReturnCode {
    Accepted = 0x00,
    UnacceptableProtocol = 0x01,
    IdentifierRejected = 0x02,
    ServerUnavailable = 0x03,
    BadCredentials = 0x04,
    NotAuthorized = 0x05,
}

impl TryFrom<u8> for ConnectReturnCode {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::Accepted),
            0x01 => Ok(Self::UnacceptableProtocol),
            0x02 => Ok(Self::IdentifierRejected),
            0x03 => Ok(Self::ServerUnavailable),
            0x04 => Ok(Self::BadCredentials),
            0x05 => Ok(Self::NotAuthorized),
            v => Err(v),
        }
    }
}

impl From<ConnectReturnCode> for u8 {
    fn from(value: ConnectReturnCode) -> Self {
        value as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SubAckReturnCode {
    SuccessQoS0 = 0x00,
    SuccessQoS1 = 0x01,
    SuccessQoS2 = 0x02,
    Failure = 0x80,
}

impl TryFrom<u8> for SubAckReturnCode {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, u8> {
        match value {
            0x00 => Ok(Self::SuccessQoS0),
            0x01 => Ok(Self::SuccessQoS1),
            0x02 => Ok(Self::SuccessQoS2),
            0x80 => Ok(Self::Failure),
            v => Err(v),
        }
    }
}

impl From<SubAckReturnCode> for u8 {
    fn from(value: SubAckReturnCode) -> Self {
        value as u8
    }
}

fn read_u16(buf: &[u8], offset: usize) -> Result<u16> {
    if buf.len() < offset + 2 {
        return Err(DecodeError::BufferTooShort {
            expected: offset + 2,
            actual: buf.len(),
        });
    }
    Ok(u16::from_be_bytes([buf[offset], buf[offset + 1]]))
}

fn read_string(buf: &[u8], offset: usize) -> Result<(String, usize)> {
    let len = read_u16(buf, offset)? as usize;
    let end = offset + 2 + len;

    if buf.len() < end {
        return Err(DecodeError::BufferTooShort {
            expected: end,
            actual: buf.len(),
        });
    }

    let s = std::str::from_utf8(&buf[offset + 2..end])
        .map_err(|_| DecodeError::InvalidUtf8)?
        .to_string();

    Ok((s, end))
}

fn write_string(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    buf.extend((bytes.len() as u16).to_be_bytes());
    buf.extend(bytes);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connect {
    pub client_id: String,
    pub keep_alive: u16,
    pub clean_session: bool,
    pub username: Option<String>,
    pub password: Option<Vec<u8>>,
}

impl Connect {
    pub fn new(client_id: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            keep_alive: 60,
            clean_session: true,
            username: None,
            password: None,
        }
    }

    fn flags(&self) -> u8 {
        let mut flags = 0u8;
        if self.clean_session {
            flags |= 0x02;
        }
        if self.username.is_some() {
            flags |= 0x80;
        }
        if self.password.is_some() {
            flags |= 0x40;
        }
        flags
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnAck {
    pub session_present: bool,
    pub return_code: ConnectReturnCode,
}

impl ConnAck {
    pub fn accepted(session_present: bool) -> Self {
        Self {
            session_present,
            return_code: ConnectReturnCode::Accepted,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Publish {
    pub topic: String,
    pub qos: QoS,
    pub retain: bool,
    pub payload: Vec<u8>,
}

impl Publish {
    pub fn new(topic: impl Into<String>, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            topic: topic.into(),
            qos: QoS::AtMostOnce,
            retain: false,
            payload: payload.into(),
        }
    }

    pub fn with_qos(mut self, qos: QoS) -> Self {
        self.qos = qos;
        self
    }

    pub fn with_retain(mut self, retain: bool) -> Self {
        self.retain = retain;
        self
    }

    fn flags(&self) -> u8 {
        let mut flags = (self.qos as u8) << 1;
        if self.retain {
            flags |= 0x01;
        }
        flags
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PubAck;

// ============================================================================
// Subscribe Packet
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscribeFilter {
    pub topic: String,
    pub qos: QoS,
}

impl SubscribeFilter {
    pub fn new(topic: impl Into<String>, qos: QoS) -> Self {
        Self {
            topic: topic.into(),
            qos,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscribe {
    pub filters: Vec<SubscribeFilter>,
}

impl Subscribe {
    pub fn new(filters: Vec<SubscribeFilter>) -> Self {
        Self { filters }
    }

    pub fn single(topic: impl Into<String>, qos: QoS) -> Self {
        Self {
            filters: vec![SubscribeFilter::new(topic, qos)],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubAck {
    pub return_codes: Vec<SubAckReturnCode>,
}

impl SubAck {
    pub fn new(return_codes: Vec<SubAckReturnCode>) -> Self {
        Self { return_codes }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PingReq;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PingResp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Disconnect;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Packet {
    Connect(Connect),
    ConnAck(ConnAck),
    Publish(Publish),
    PubAck(PubAck),
    Subscribe(Subscribe),
    SubAck(SubAck),
    PingReq(PingReq),
    PingResp(PingResp),
    Disconnect(Disconnect),
}

impl Packet {
    pub fn msg_type(&self) -> MessageType {
        match self {
            Self::Connect(_) => MessageType::Connect,
            Self::ConnAck(_) => MessageType::ConnAck,
            Self::Publish(_) => MessageType::Publish,
            Self::PubAck(_) => MessageType::PubAck,
            Self::Subscribe(_) => MessageType::Subscribe,
            Self::SubAck(_) => MessageType::SubAck,
            Self::PingReq(_) => MessageType::PingReq,
            Self::PingResp(_) => MessageType::PingResp,
            Self::Disconnect(_) => MessageType::Disconnect,
        }
    }
}

impl From<Connect> for Packet {
    fn from(p: Connect) -> Self {
        Self::Connect(p)
    }
}

impl From<ConnAck> for Packet {
    fn from(p: ConnAck) -> Self {
        Self::ConnAck(p)
    }
}

impl From<Publish> for Packet {
    fn from(p: Publish) -> Self {
        Self::Publish(p)
    }
}

impl From<PubAck> for Packet {
    fn from(p: PubAck) -> Self {
        Self::PubAck(p)
    }
}

impl From<Subscribe> for Packet {
    fn from(p: Subscribe) -> Self {
        Self::Subscribe(p)
    }
}

impl From<SubAck> for Packet {
    fn from(p: SubAck) -> Self {
        Self::SubAck(p)
    }
}

impl From<PingReq> for Packet {
    fn from(p: PingReq) -> Self {
        Self::PingReq(p)
    }
}

impl From<PingResp> for Packet {
    fn from(p: PingResp) -> Self {
        Self::PingResp(p)
    }
}

impl From<Disconnect> for Packet {
    fn from(p: Disconnect) -> Self {
        Self::Disconnect(p)
    }
}

// ============================================================================
// UdpFrame (Wire Format with Header)
// ============================================================================

/// Wire format:
/// | Length (1) | Type (1) | MsgID (2) | Payload (N) |
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UdpFrame {
    pub msg_id: u16,
    pub packet: Packet,
}

impl UdpFrame {
    pub const HEADER_LEN: usize = 4;
    pub const MAX_PACKET_LEN: usize = 255;

    pub fn new(msg_id: u16, packet: impl Into<Packet>) -> Self {
        Self {
            msg_id,
            packet: packet.into(),
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let payload_len = self.packet_payload_len();
        let total_len = Self::HEADER_LEN + payload_len;

        let mut buf = Vec::with_capacity(total_len);
        buf.push(total_len as u8);
        buf.push(self.packet.msg_type().into());
        buf.extend(self.msg_id.to_be_bytes());

        match &self.packet {
            Packet::Connect(p) => p.encode(&mut buf),
            Packet::ConnAck(p) => p.encode(&mut buf),
            Packet::Publish(p) => p.encode(&mut buf),
            Packet::PubAck(p) => p.encode(&mut buf),
            Packet::Subscribe(p) => p.encode(&mut buf),
            Packet::SubAck(p) => p.encode(&mut buf),
            Packet::PingReq(p) => p.encode(&mut buf),
            Packet::PingResp(p) => p.encode(&mut buf),
            Packet::Disconnect(p) => p.encode(&mut buf),
        }

        buf
    }

    pub fn decode(buf: &[u8]) -> Result<Self> {
        if buf.len() < Self::HEADER_LEN {
            return Err(DecodeError::BufferTooShort {
                expected: Self::HEADER_LEN,
                actual: buf.len(),
            });
        }

        let length = buf[0] as usize;
        if buf.len() < length {
            return Err(DecodeError::BufferTooShort {
                expected: length,
                actual: buf.len(),
            });
        }

        let msg_type = MessageType::try_from(buf[1])?;
        let msg_id = u16::from_be_bytes([buf[2], buf[3]]);
        let payload = &buf[Self::HEADER_LEN..length];

        let packet = match msg_type {
            MessageType::Connect => Packet::Connect(Connect::decode(payload)?),
            MessageType::ConnAck => Packet::ConnAck(ConnAck::decode(payload)?),
            MessageType::Publish => Packet::Publish(Publish::decode(payload)?),
            MessageType::PubAck => Packet::PubAck(PubAck::decode(payload)?),
            MessageType::Subscribe => Packet::Subscribe(Subscribe::decode(payload)?),
            MessageType::SubAck => Packet::SubAck(SubAck::decode(payload)?),
            MessageType::PingReq => Packet::PingReq(PingReq::decode(payload)?),
            MessageType::PingResp => Packet::PingResp(PingResp::decode(payload)?),
            MessageType::Disconnect => Packet::Disconnect(Disconnect::decode(payload)?),
        };

        Ok(Self { msg_id, packet })
    }

    fn packet_payload_len(&self) -> usize {
        match &self.packet {
            Packet::Connect(p) => p.encoded_len(),
            Packet::ConnAck(p) => p.encoded_len(),
            Packet::Publish(p) => p.encoded_len(),
            Packet::PubAck(p) => p.encoded_len(),
            Packet::Subscribe(p) => p.encoded_len(),
            Packet::SubAck(p) => p.encoded_len(),
            Packet::PingReq(p) => p.encoded_len(),
            Packet::PingResp(p) => p.encoded_len(),
            Packet::Disconnect(p) => p.encoded_len(),
        }
    }
}
