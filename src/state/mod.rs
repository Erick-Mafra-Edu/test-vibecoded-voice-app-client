//! Application state — single source of truth for PULSE.

use std::collections::VecDeque;

// ─────────────────────────────────────────────────────────────
//  Server & Channel
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Server {
    pub id: u32,
    pub name: String,
    pub icon: char,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChannelKind {
    Voice,
    Text,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    pub id: u32,
    pub name: String,
    pub kind: ChannelKind,
    pub user_count: usize,
}

// ─────────────────────────────────────────────────────────────
//  Participant
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Participant {
    pub id: u32,
    pub display_name: String,
    pub avatar_color: (u8, u8, u8), // RGB for generated avatar
    pub is_muted: bool,
    pub is_deafened: bool,
    pub is_speaking: bool,
    pub speaking_level: f32, // 0.0 – 1.0 for ring animation
    pub is_screen_sharing: bool,
    pub is_camera_on: bool,
    pub ping_ms: u32,
}

impl Participant {
    pub fn initials(&self) -> String {
        self.display_name
            .split_whitespace()
            .take(2)
            .filter_map(|w| w.chars().next())
            .collect::<String>()
            .to_uppercase()
    }
}

// ─────────────────────────────────────────────────────────────
//  Chat messages
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub author: String,
    pub content: String,
    pub timestamp: String,
    pub is_self: bool,
}

// ─────────────────────────────────────────────────────────────
//  Network metrics (HUD)
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct NetworkMetrics {
    pub latency_ms: u32,
    pub jitter_ms: u32,
    pub packet_loss_pct: f32,
    pub upload_kbps: u32,
    pub download_kbps: u32,
}

impl NetworkMetrics {
    pub fn quality_label(&self) -> &'static str {
        if self.latency_ms < 50 && self.packet_loss_pct < 0.5 {
            "Excellent"
        } else if self.latency_ms < 100 && self.packet_loss_pct < 2.0 {
            "Good"
        } else if self.latency_ms < 200 {
            "Fair"
        } else {
            "Poor"
        }
    }
}

// ─────────────────────────────────────────────────────────────
//  Local user
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LocalUser {
    pub display_name: String,
    pub avatar_color: (u8, u8, u8),
    pub is_muted: bool,
    pub is_deafened: bool,
    pub is_screen_sharing: bool,
}

#[derive(Debug, Clone, Default)]
pub struct MediaState {
    pub input_devices: Vec<String>,
    pub output_devices: Vec<String>,
    pub selected_input: Option<String>,
    pub selected_output: Option<String>,
    pub capture_active: bool,
    pub screens: Vec<String>,
    pub selected_screen: usize,
    pub screen_capture_active: bool,
    pub last_error: Option<String>,
}

impl LocalUser {
    pub fn initials(&self) -> String {
        self.display_name
            .split_whitespace()
            .take(2)
            .filter_map(|w| w.chars().next())
            .collect::<String>()
            .to_uppercase()
    }
}

// ─────────────────────────────────────────────────────────────
//  Navigation
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default)]
pub enum NavTab {
    #[default]
    Home,
    Servers,
    Profile,
    Settings,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum AppMode {
    #[default]
    Normal,
    ScreenShare,
    Overlay,
}

// ─────────────────────────────────────────────────────────────
//  Root state
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AppState {
    // Navigation
    pub nav_tab: NavTab,
    pub mode: AppMode,

    // Server / channel selection
    pub servers: Vec<Server>,
    pub selected_server: usize,
    pub voice_channels: Vec<Channel>,
    pub text_channels: Vec<Channel>,
    pub active_voice_channel: usize,
    pub active_text_channel: usize,
    pub joined_voice: bool,

    // Participants in the active voice channel
    pub participants: Vec<Participant>,

    // Local user
    pub local_user: LocalUser,
    pub media: MediaState,

    // Authentication test tokens
    pub auth_tokens: Vec<String>,
    pub selected_auth_token: Option<String>,
    pub auth_token_input: String,
    pub auth_token_error: Option<String>,

    // Chat
    pub chat_messages: VecDeque<ChatMessage>,
    pub chat_input: String,
    pub chat_visible: bool,

    // Dock
    pub dock_expanded: bool,

    // Network
    pub metrics: NetworkMetrics,

    // Tick counter for animations
    pub tick: u64,
}

impl Default for AppState {
    fn default() -> Self {
        let configured_token = std::env::var("PULSE_AUTH_TOKEN").ok();
        let mut auth_tokens = Vec::new();
        for key in ["PULSE_AUTH_TOKEN_1", "PULSE_AUTH_TOKEN_2"] {
            if let Ok(token) = std::env::var(key) {
                if !token.trim().is_empty() && !auth_tokens.contains(&token) {
                    auth_tokens.push(token);
                }
            }
        }
        if let Some(token) = configured_token.clone() {
            if !token.trim().is_empty() && !auth_tokens.contains(&token) {
                auth_tokens.insert(0, token);
            }
        }

        let servers = vec![
            Server {
                id: 1,
                name: "Squad Principal".into(),
                icon: '◈',
            },
            Server {
                id: 2,
                name: "Ghost Protocol".into(),
                icon: '◌',
            },
            Server {
                id: 3,
                name: "Apex Legends EU".into(),
                icon: '◆',
            },
        ];

        let voice_channels = vec![
            Channel {
                id: 1,
                name: "Quads Central".into(),
                kind: ChannelKind::Voice,
                user_count: 4,
            },
            Channel {
                id: 2,
                name: "Game Chat".into(),
                kind: ChannelKind::Voice,
                user_count: 2,
            },
        ];

        let text_channels = vec![
            Channel {
                id: 10,
                name: "General".into(),
                kind: ChannelKind::Text,
                user_count: 0,
            },
            Channel {
                id: 11,
                name: "Clips".into(),
                kind: ChannelKind::Text,
                user_count: 0,
            },
            Channel {
                id: 12,
                name: "Strategy".into(),
                kind: ChannelKind::Text,
                user_count: 0,
            },
        ];

        let participants = vec![
            Participant {
                id: 2,
                display_name: "Alpha".into(),
                avatar_color: (0, 217, 245),
                is_muted: false,
                is_deafened: false,
                is_speaking: true,
                speaking_level: 0.8,
                is_screen_sharing: false,
                is_camera_on: false,
                ping_ms: 23,
            },
            Participant {
                id: 3,
                display_name: "Bravo".into(),
                avatar_color: (255, 181, 71),
                is_muted: false,
                is_deafened: false,
                is_speaking: true,
                speaking_level: 0.7,
                is_screen_sharing: false,
                is_camera_on: true,
                ping_ms: 47,
            },
            Participant {
                id: 4,
                display_name: "Charlie".into(),
                avatar_color: (50, 205, 99),
                is_muted: false,
                is_deafened: false,
                is_speaking: false,
                speaking_level: 0.3,
                is_screen_sharing: true,
                is_camera_on: false,
                ping_ms: 89,
            },
            Participant {
                id: 5,
                display_name: "Delta".into(),
                avatar_color: (180, 80, 220),
                is_muted: false,
                is_deafened: true,
                is_speaking: false,
                speaking_level: 0.0,
                is_screen_sharing: false,
                is_camera_on: false,
                ping_ms: 112,
            },
        ];

        let chat_messages = {
            let mut q = VecDeque::new();
            q.push_back(ChatMessage {
                author: "Vexx".into(),
                content: "Prontos para a próxima partida?".into(),
                timestamp: "10:24".into(),
                is_self: true,
            });
            q.push_back(ChatMessage {
                author: "Alpha".into(),
                content: "Bora, só mais 2 min.".into(),
                timestamp: "10:24".into(),
                is_self: false,
            });
            q.push_back(ChatMessage {
                author: "Bravo".into(),
                content: "Vou pegar um café.".into(),
                timestamp: "10:25".into(),
                is_self: false,
            });
            q.push_back(ChatMessage {
                author: "Charlie".into(),
                content: "Copy.".into(),
                timestamp: "10:26".into(),
                is_self: false,
            });
            q.push_back(ChatMessage {
                author: "Delta".into(),
                content: "Já volto.".into(),
                timestamp: "10:26".into(),
                is_self: false,
            });
            q
        };

        Self {
            nav_tab: NavTab::Home,
            mode: AppMode::Normal,
            servers,
            selected_server: 0,
            voice_channels,
            text_channels,
            active_voice_channel: 0,
            active_text_channel: 0,
            joined_voice: true,
            participants,
            local_user: LocalUser {
                display_name: "Vexx".into(),
                avatar_color: (0, 217, 245),
                is_muted: false,
                is_deafened: false,
                is_screen_sharing: false,
            },
            media: MediaState::default(),
            selected_auth_token: configured_token,
            auth_tokens,
            auth_token_input: String::new(),
            auth_token_error: None,
            chat_messages,
            chat_input: String::new(),
            chat_visible: true,
            dock_expanded: true,
            metrics: NetworkMetrics {
                latency_ms: 18,
                jitter_ms: 2,
                packet_loss_pct: 0.0,
                upload_kbps: 64,
                download_kbps: 320,
            },
            tick: 0,
        }
    }
}
