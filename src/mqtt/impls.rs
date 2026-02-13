use super::*;

impl Encode for Connect {
    fn encode(&self, buf: &mut Vec<u8>) {
        buf.push(self.flags());
        buf.extend(self.keep_alive.to_be_bytes());
        write_string(buf, &self.client_id);

        if let Some(ref username) = self.username {
            write_string(buf, username);
        }
        if let Some(ref password) = self.password {
            buf.extend((password.len() as u16).to_be_bytes());
            buf.extend(password);
        }
    }

    fn encoded_len(&self) -> usize {
        let mut len = 1 + 2 + 2 + self.client_id.len();
        if let Some(ref u) = self.username {
            len += 2 + u.len();
        }
        if let Some(ref p) = self.password {
            len += 2 + p.len();
        }
        len
    }
}

impl Decode for Connect {
    fn decode(buf: &[u8]) -> Result<Self> {
        if buf.len() < 3 {
            return Err(DecodeError::BufferTooShort {
                expected: 3,
                actual: buf.len(),
            });
        }

        let flags = buf[0];
        let clean_session = (flags & 0x02) != 0;
        let has_username = (flags & 0x80) != 0;
        let has_password = (flags & 0x40) != 0;

        let keep_alive = read_u16(buf, 1)?;
        let (client_id, mut offset) = read_string(buf, 3)?;

        let username = if has_username {
            let (u, new_offset) = read_string(buf, offset)?;
            offset = new_offset;
            Some(u)
        } else {
            None
        };

        let password = if has_password {
            let len = read_u16(buf, offset)? as usize;
            let end = offset + 2 + len;
            if buf.len() < end {
                return Err(DecodeError::BufferTooShort {
                    expected: end,
                    actual: buf.len(),
                });
            }
            Some(buf[offset + 2..end].to_vec())
        } else {
            None
        };

        Ok(Self {
            client_id,
            keep_alive,
            clean_session,
            username,
            password,
        })
    }
}

impl Encode for ConnAck {
    fn encode(&self, buf: &mut Vec<u8>) {
        buf.push(self.session_present as u8);
        buf.push(self.return_code.into());
    }

    fn encoded_len(&self) -> usize {
        2
    }
}

impl Decode for ConnAck {
    fn decode(buf: &[u8]) -> Result<Self> {
        if buf.len() < 2 {
            return Err(DecodeError::BufferTooShort {
                expected: 2,
                actual: buf.len(),
            });
        }
        Ok(Self {
            session_present: buf[0] != 0,
            return_code: ConnectReturnCode::try_from(buf[1])
                .map_err(DecodeError::InvalidReturnCode)?,
        })
    }
}

impl Encode for Publish {
    fn encode(&self, buf: &mut Vec<u8>) {
        buf.push(self.flags());
        write_string(buf, &self.topic);
        buf.extend(&self.payload);
    }

    fn encoded_len(&self) -> usize {
        1 + 2 + self.topic.len() + self.payload.len()
    }
}

impl Decode for Publish {
    fn decode(buf: &[u8]) -> Result<Self> {
        if buf.is_empty() {
            return Err(DecodeError::BufferTooShort {
                expected: 1,
                actual: 0,
            });
        }

        let flags = buf[0];
        let qos = QoS::try_from((flags >> 1) & 0x03).map_err(DecodeError::InvalidQoS)?;
        let retain = (flags & 0x01) != 0;

        let (topic, offset) = read_string(buf, 1)?;
        let payload = buf[offset..].to_vec();

        Ok(Self {
            topic,
            qos,
            retain,
            payload,
        })
    }
}

impl Encode for PubAck {
    fn encode(&self, _buf: &mut Vec<u8>) {}

    fn encoded_len(&self) -> usize {
        0
    }
}

impl Decode for PubAck {
    fn decode(_buf: &[u8]) -> Result<Self> {
        Ok(Self)
    }
}

impl Encode for Subscribe {
    fn encode(&self, buf: &mut Vec<u8>) {
        buf.push(self.filters.len() as u8);
        for filter in &self.filters {
            write_string(buf, &filter.topic);
            buf.push(filter.qos.into());
        }
    }

    fn encoded_len(&self) -> usize {
        1 + self
            .filters
            .iter()
            .map(|f| 2 + f.topic.len() + 1)
            .sum::<usize>()
    }
}

impl Decode for Subscribe {
    fn decode(buf: &[u8]) -> Result<Self> {
        if buf.is_empty() {
            return Err(DecodeError::BufferTooShort {
                expected: 1,
                actual: 0,
            });
        }

        let count = buf[0] as usize;
        let mut filters = Vec::with_capacity(count);
        let mut offset = 1;

        for _ in 0..count {
            let (topic, new_offset) = read_string(buf, offset)?;
            if buf.len() <= new_offset {
                return Err(DecodeError::BufferTooShort {
                    expected: new_offset + 1,
                    actual: buf.len(),
                });
            }
            let qos = QoS::try_from(buf[new_offset]).map_err(DecodeError::InvalidQoS)?;
            offset = new_offset + 1;
            filters.push(SubscribeFilter { topic, qos });
        }

        Ok(Self { filters })
    }
}

impl Encode for SubAck {
    fn encode(&self, buf: &mut Vec<u8>) {
        buf.push(self.return_codes.len() as u8);
        for code in &self.return_codes {
            buf.push((*code).into());
        }
    }

    fn encoded_len(&self) -> usize {
        1 + self.return_codes.len()
    }
}

impl Decode for SubAck {
    fn decode(buf: &[u8]) -> Result<Self> {
        if buf.is_empty() {
            return Err(DecodeError::BufferTooShort {
                expected: 1,
                actual: 0,
            });
        }

        let count = buf[0] as usize;
        if buf.len() < 1 + count {
            return Err(DecodeError::BufferTooShort {
                expected: 1 + count,
                actual: buf.len(),
            });
        }

        let return_codes = buf[1..1 + count]
            .iter()
            .map(|&b| SubAckReturnCode::try_from(b).map_err(DecodeError::InvalidReturnCode))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self { return_codes })
    }
}

impl Encode for PingReq {
    fn encode(&self, _buf: &mut Vec<u8>) {}
    fn encoded_len(&self) -> usize {
        0
    }
}

impl Decode for PingReq {
    fn decode(_buf: &[u8]) -> Result<Self> {
        Ok(Self)
    }
}

impl Encode for PingResp {
    fn encode(&self, _buf: &mut Vec<u8>) {}
    fn encoded_len(&self) -> usize {
        0
    }
}

impl Decode for PingResp {
    fn decode(_buf: &[u8]) -> Result<Self> {
        Ok(Self)
    }
}

impl Encode for Disconnect {
    fn encode(&self, _buf: &mut Vec<u8>) {}
    fn encoded_len(&self) -> usize {
        0
    }
}

impl Decode for Disconnect {
    fn decode(_buf: &[u8]) -> Result<Self> {
        Ok(Self)
    }
}
