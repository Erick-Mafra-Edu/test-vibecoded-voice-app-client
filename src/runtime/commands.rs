//! Commands accepted by the client runtime.

#[derive(Debug, Clone)]
pub enum ClientCommand {
    JoinVoiceChannel { channel_id: u32 },
    LeaveVoiceChannel,
    SetMuted(bool),
    SetDeafened(bool),
    SetScreenSharing(bool),
    SetInputDevice(String),
    SetOutputDevice(String),
    SetScreenDevice(usize),
    RefreshMediaDevices,
    SendChat { content: String },
    SetAuthToken(String),
}
