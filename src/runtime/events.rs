//! Events emitted by the client runtime.

use crate::state::{ChatMessage, NetworkMetrics, Participant};

#[derive(Debug, Clone)]
pub enum ClientEvent {
    Connected,
    VoiceJoined { channel_id: u32 },
    VoiceLeft,
    Snapshot(RuntimeSnapshot),
    MediaError(String),
}

#[derive(Debug, Clone)]
pub struct RuntimeSnapshot {
    pub tick: u64,
    pub joined_voice: bool,
    pub participants: Vec<Participant>,
    pub metrics: NetworkMetrics,
    pub chat_messages: Vec<ChatMessage>,
    pub media: crate::state::MediaState,
}
