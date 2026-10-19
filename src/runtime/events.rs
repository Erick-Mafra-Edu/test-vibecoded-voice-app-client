//! Events emitted by the client runtime.

use crate::state::{ChatMessage, LocalUser, NetworkMetrics, Participant};

#[derive(Debug, Clone)]
pub enum ClientEvent {
    Connected,
    SimulationMode,
    ConnectionFailed(String),
    VoiceJoined { channel_id: u32 },
    VoiceLeft,
    Snapshot(RuntimeSnapshot),
    MediaError(String),
    ChatError(String),
}

#[derive(Debug, Clone)]
pub struct RuntimeSnapshot {
    pub backend_connected: bool,
    pub tick: u64,
    pub joined_voice: bool,
    pub local_user: LocalUser,
    pub participants: Vec<Participant>,
    pub metrics: NetworkMetrics,
    pub chat_messages: Vec<ChatMessage>,
    pub media: crate::state::MediaState,
}
