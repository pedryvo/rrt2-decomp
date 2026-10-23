//! Reliability above an unreliable transport (the role of `0x4a0320`/`0x4a1210` in the original,
//! simplified: a frame is a whole message, there is no fragmentation).
//!
//! Frame: `tag u8` then, for `0` raw (not acknowledged), the payload; for `1` data, `id u32` and
//! the payload; for `2` acknowledgement, `id u32`. A data frame is resent until acknowledged;
//! the receiver delivers each id once, in arrival order, and acknowledges every copy.

use std::collections::{BTreeMap, BTreeSet};

use crate::NetError;

const F_RAW: u8 = 0;
const F_DATA: u8 = 1;
const F_ACK: u8 = 2;

struct Unacked {
    frame: Vec<u8>,
    last_ms: u64,
}

/// The state of one link.
pub struct Reliable {
    next_id: u32,
    unacked: BTreeMap<u32, Unacked>,
    /// Every id below `floor` was delivered.
    floor: u32,
    /// Delivered ids at or above `floor`.
    seen: BTreeSet<u32>,
}

impl Default for Reliable {
    fn default() -> Self {
        Reliable {
            next_id: 1,
            unacked: BTreeMap::new(),
            floor: 1,
            seen: BTreeSet::new(),
        }
    }
}

/// What [`Reliable::on_frame`] found.
pub struct Received {
    /// The payload to deliver (none for an acknowledgement or a duplicate).
    pub payload: Option<Vec<u8>>,
    /// A frame to send back.
    pub reply: Option<Vec<u8>>,
}

impl Reliable {
    /// Frames an unacknowledged payload (heartbeats, pings).
    pub fn raw(payload: &[u8]) -> Vec<u8> {
        let mut f = Vec::with_capacity(payload.len() + 1);
        f.push(F_RAW);
        f.extend_from_slice(payload);
        f
    }

    /// Frames a payload that must arrive, and remembers it for resending.
    pub fn data(&mut self, payload: &[u8], now_ms: u64) -> Vec<u8> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let mut f = Vec::with_capacity(payload.len() + 5);
        f.push(F_DATA);
        f.extend_from_slice(&id.to_le_bytes());
        f.extend_from_slice(payload);
        self.unacked.insert(
            id,
            Unacked {
                frame: f.clone(),
                last_ms: now_ms,
            },
        );
        f
    }

    pub fn on_frame(&mut self, frame: &[u8]) -> Result<Received, NetError> {
        let (&tag, rest) = frame
            .split_first()
            .ok_or(NetError::Malformed("empty frame"))?;
        match tag {
            F_RAW => Ok(Received {
                payload: Some(rest.to_vec()),
                reply: None,
            }),
            F_DATA | F_ACK => {
                let id = u32::from_le_bytes(
                    rest.get(..4)
                        .ok_or(NetError::Malformed("short frame"))?
                        .try_into()
                        .unwrap(),
                );
                if tag == F_ACK {
                    self.unacked.remove(&id);
                    return Ok(Received {
                        payload: None,
                        reply: None,
                    });
                }
                let mut ack = vec![F_ACK];
                ack.extend_from_slice(&id.to_le_bytes());
                let dup = id < self.floor || self.seen.contains(&id);
                if !dup {
                    self.seen.insert(id);
                    while self.seen.remove(&self.floor) {
                        self.floor = self.floor.wrapping_add(1);
                    }
                }
                Ok(Received {
                    payload: (!dup).then(|| rest[4..].to_vec()),
                    reply: Some(ack),
                })
            }
            _ => Err(NetError::Malformed("unknown frame tag")),
        }
    }

    /// The frames to send again: those unacknowledged for `resend_ms`.
    pub fn due(&mut self, now_ms: u64, resend_ms: u64) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        for u in self.unacked.values_mut() {
            if now_ms.saturating_sub(u.last_ms) >= resend_ms {
                u.last_ms = now_ms;
                out.push(u.frame.clone());
            }
        }
        out
    }

    pub fn unacked(&self) -> usize {
        self.unacked.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_is_delivered_once_and_acknowledged_every_time() {
        let (mut a, mut b) = (Reliable::default(), Reliable::default());
        let f = a.data(b"hello", 0);
        let r1 = b.on_frame(&f).unwrap();
        assert_eq!(r1.payload.as_deref(), Some(&b"hello"[..]));
        let r2 = b.on_frame(&f).unwrap();
        assert_eq!(r2.payload, None, "duplicate");
        assert!(r2.reply.is_some(), "but acknowledged again");
        assert_eq!(a.unacked(), 1);
        a.on_frame(&r1.reply.unwrap()).unwrap();
        assert_eq!(a.unacked(), 0);
    }

    #[test]
    fn lost_frames_are_resent_after_the_interval() {
        let mut a = Reliable::default();
        a.data(b"x", 100);
        assert!(a.due(150, 100).is_empty());
        assert_eq!(a.due(200, 100).len(), 1);
        assert!(a.due(250, 100).is_empty(), "the clock restarts");
        assert_eq!(a.due(300, 100).len(), 1);
    }

    #[test]
    fn out_of_order_ids_advance_the_floor() {
        let (mut a, mut b) = (Reliable::default(), Reliable::default());
        let f: Vec<_> = (0..4u8).map(|i| a.data(&[i], 0)).collect();
        for i in [2, 0, 3, 1] {
            assert_eq!(b.on_frame(&f[i]).unwrap().payload, Some(vec![i as u8]));
        }
        assert!(b.seen.is_empty(), "contiguous ids collapse into the floor");
        assert_eq!(b.floor, 5);
    }

    #[test]
    fn raw_frames_pass_through() {
        let mut b = Reliable::default();
        let r = b.on_frame(&Reliable::raw(b"hb")).unwrap();
        assert_eq!(r.payload.as_deref(), Some(&b"hb"[..]));
        assert!(r.reply.is_none());
        assert!(b.on_frame(&[]).is_err());
        assert!(b.on_frame(&[9]).is_err());
        assert!(b.on_frame(&[F_DATA, 1]).is_err());
    }
}
