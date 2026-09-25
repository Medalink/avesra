//! Binary audio boundary; only an already authenticated media session may use it.
use crate::{ErrorCode, MAX_AUDIO_FRAME_BYTES};
/// Media framing evolves independently of the control handshake.
pub const MEDIA_PROTOCOL_VERSION: u16 = 1;
use uuid::Uuid;

const HEADER: usize = 82;
pub const MAX_PACKET: usize = HEADER + MAX_AUDIO_FRAME_BYTES;
pub struct AudioPacket {
    pub device_id: Uuid,
    pub session_id: Uuid,
    pub utterance_id: Uuid,
    pub capture_epoch: u64,
    pub sequence: u64,
    pub sample_offset: u64,
    pub start: bool,
    pub end: bool,
    pub pcm: Vec<u8>,
}
impl AudioPacket {
    pub fn validate_shape(&self) -> Result<(), ErrorCode> {
        if [self.device_id, self.session_id, self.utterance_id]
            .iter()
            .any(Uuid::is_nil)
            || self.capture_epoch == 0
            || self.sequence == 0
            || self.pcm.is_empty()
            || self.pcm.len() > MAX_AUDIO_FRAME_BYTES
            || !self.pcm.len().is_multiple_of(2)
            || self.sample_offset > 480_000
            || self.sample_offset + self.pcm.len() as u64 / 2 > 480_000
            || self.start && self.sample_offset != 0
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<Vec<u8>, ErrorCode> {
        self.validate_shape()?;
        let mut bytes = Vec::with_capacity(HEADER + self.pcm.len());
        bytes.extend_from_slice(b"AVAU");
        bytes.extend_from_slice(&MEDIA_PROTOCOL_VERSION.to_le_bytes());
        bytes
            .extend_from_slice(&(u16::from(self.start) | (u16::from(self.end) << 1)).to_le_bytes());
        for id in [self.device_id, self.session_id, self.utterance_id] {
            bytes.extend_from_slice(id.as_bytes());
        }
        for value in [self.capture_epoch, self.sequence, self.sample_offset] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&(self.pcm.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&self.pcm);
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, ErrorCode> {
        if bytes.len() < HEADER || bytes.len() > MAX_PACKET {
            return Err(ErrorCode::TooLarge);
        }
        if &bytes[..4] != b"AVAU" {
            return Err(ErrorCode::Malformed);
        }
        let mut offset = 4;
        fn take<const N: usize>(bytes: &[u8], offset: &mut usize) -> Result<[u8; N], ErrorCode> {
            let value = bytes
                .get(*offset..*offset + N)
                .ok_or(ErrorCode::Malformed)?
                .try_into()
                .map_err(|_| ErrorCode::Malformed)?;
            *offset += N;
            Ok(value)
        }
        if u16::from_le_bytes(take(bytes, &mut offset)?) != MEDIA_PROTOCOL_VERSION {
            return Err(ErrorCode::Version);
        }
        let flags = u16::from_le_bytes(take(bytes, &mut offset)?);
        if flags & !3 != 0 {
            return Err(ErrorCode::Malformed);
        }
        let device_id = Uuid::from_bytes(take(bytes, &mut offset)?);
        let session_id = Uuid::from_bytes(take(bytes, &mut offset)?);
        let utterance_id = Uuid::from_bytes(take(bytes, &mut offset)?);
        let capture_epoch = u64::from_le_bytes(take(bytes, &mut offset)?);
        let sequence = u64::from_le_bytes(take(bytes, &mut offset)?);
        let sample_offset = u64::from_le_bytes(take(bytes, &mut offset)?);
        let length = usize::from(u16::from_le_bytes(take(bytes, &mut offset)?));
        if length != bytes.len() - HEADER {
            return Err(ErrorCode::Malformed);
        }
        let packet = Self {
            device_id,
            session_id,
            utterance_id,
            capture_epoch,
            sequence,
            sample_offset,
            start: flags & 1 != 0,
            end: flags & 2 != 0,
            pcm: bytes[HEADER..].to_vec(),
        };
        packet.validate_shape()?;
        Ok(packet)
    }
}

/// The controller constructs this from authenticated pairing/owner setup and
/// current local modes. Binary contents never supply their own admission rights.
pub struct MediaSession {
    pub device_id: Uuid,
    pub session_id: Uuid,
    pub capture_epoch: u64,
    pub enabled: bool,
    last_sequence: u64,
    utterance: Option<Uuid>,
    next_offset: u64,
    started: Option<std::time::Instant>,
    recent: std::collections::VecDeque<(Uuid, std::time::Instant)>,
}
impl MediaSession {
    pub fn new(device_id: Uuid, session_id: Uuid, capture_epoch: u64) -> Result<Self, ErrorCode> {
        if device_id.is_nil() || session_id.is_nil() || capture_epoch == 0 {
            return Err(ErrorCode::Malformed);
        }
        Ok(Self {
            device_id,
            session_id,
            capture_epoch,
            enabled: false,
            last_sequence: 0,
            utterance: None,
            next_offset: 0,
            started: None,
            recent: std::collections::VecDeque::new(),
        })
    }
    pub fn invalidate(&mut self, epoch: u64) -> Result<(), ErrorCode> {
        self.enabled = false;
        if epoch <= self.capture_epoch {
            return Err(ErrorCode::Stale);
        }
        self.capture_epoch = epoch;
        self.last_sequence = 0;
        self.utterance = None;
        self.next_offset = 0;
        self.started = None;
        Ok(())
    }
    pub fn accept(&mut self, packet: &AudioPacket) -> Result<(), ErrorCode> {
        packet.validate_shape()?;
        if !self.enabled {
            return Err(ErrorCode::Unauthenticated);
        }
        if self
            .started
            .is_some_and(|started| started.elapsed() > std::time::Duration::from_secs(30))
        {
            return Err(ErrorCode::Expired);
        }
        if packet.device_id != self.device_id
            || packet.session_id != self.session_id
            || packet.capture_epoch != self.capture_epoch
            || packet.sequence != self.last_sequence.checked_add(1).ok_or(ErrorCode::Stale)?
        {
            return Err(ErrorCode::Stale);
        }
        if packet.start {
            if self.utterance.is_some() || packet.sample_offset != 0 {
                return Err(ErrorCode::InvalidTransition);
            }
            while self
                .recent
                .front()
                .is_some_and(|(_, created)| created.elapsed() > std::time::Duration::from_secs(61))
            {
                self.recent.pop_front();
            }
            if self.recent.iter().any(|(id, _)| *id == packet.utterance_id) {
                return Err(ErrorCode::Stale);
            }
            if self.recent.len() >= 128 {
                return Err(ErrorCode::Unavailable);
            }
            self.recent
                .push_back((packet.utterance_id, std::time::Instant::now()));
            self.started = Some(std::time::Instant::now());
        } else if self.utterance != Some(packet.utterance_id)
            || packet.sample_offset != self.next_offset
        {
            return Err(ErrorCode::Stale);
        }
        self.last_sequence = packet.sequence;
        self.next_offset = packet.sample_offset + packet.pcm.len() as u64 / 2;
        self.utterance = if packet.end {
            self.started = None;
            None
        } else {
            Some(packet.utterance_id)
        };
        Ok(())
    }
}
