//! Typed messages (Elm-style) for PULSE.

use crate::runtime::ClientEvent;
use crate::state::{AppMode, NavTab};

#[derive(Debug, Clone)]
pub enum Message {
    // ── Navigation ──────────────────────────────────
    NavTabSelected(NavTab),
    BackToHome,
    ServerSelected(usize),

    // ── Channels ────────────────────────────────────
    VoiceChannelSelected(usize),
    TextChannelSelected(usize),
    JoinVoice,
    LeaveVoice,

    // ── Audio controls ──────────────────────────────
    ToggleMute,
    ToggleDeafen,
    ToggleScreenShare,
    InputDeviceSelected(String),
    OutputDeviceSelected(String),
    ScreenDeviceSelected(usize),
    RefreshMediaDevices,
    AuthTokenSelected(String),
    AuthTokenInputChanged(String),
    ApplyAuthToken,
    DockToggle,

    // ── Chat ────────────────────────────────────────
    ChatInputChanged(String),
    ChatSubmit,
    ChatVisibilityToggle,

    RuntimeEvent(ClientEvent),

    // ── App mode ────────────────────────────────────
    SetMode(AppMode),
}
