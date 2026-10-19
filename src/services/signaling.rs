//! BeamVoice WebSocket signaling client.

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug, Clone)]
pub struct SignalingConfig {
    pub url: String,
    pub token: String,
    pub room: String,
}

#[derive(Debug, Clone)]
pub enum ServerEvent {
    Authenticated {
        user: String,
    },
    Joined {
        user: String,
        members: Vec<String>,
        media_key: Option<String>,
    },
    Members {
        members: Vec<String>,
    },
    PeerJoined {
        user: String,
    },
    PeerLeft {
        user: String,
    },
    Chat {
        from: String,
        content: String,
    },
    Signal(Value),
    Error {
        reason: String,
    },
}

pub struct SignalingConnection {
    socket: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
}

impl SignalingConfig {
    pub fn from_env() -> Option<Self> {
        let url = std::env::var("PULSE_SIGNAL_URL").ok()?;
        let token = std::env::var("PULSE_AUTH_TOKEN").unwrap_or_default();
        let room = std::env::var("PULSE_ROOM").ok()?;

        if url.trim().is_empty() || room.trim().is_empty() {
            return None;
        }

        Some(Self { url, token, room })
    }
}

impl SignalingConnection {
    pub async fn connect(config: &SignalingConfig) -> Result<(Self, ServerEvent), String> {
        let (socket, _) = connect_async(&config.url)
            .await
            .map_err(|error| format!("signaling connection failed: {error}"))?;
        let mut connection = Self { socket };

        connection
            .send(json!({"type": "auth", "token": config.token}))
            .await?;
        let authenticated = connection
            .next_event()
            .await?
            .ok_or_else(|| "signaling closed during authentication".to_owned())?;
        if !matches!(authenticated, ServerEvent::Authenticated { .. }) {
            return Err("authentication rejected or unexpected response".into());
        }

        connection
            .send(json!({"type": "join", "room": config.room}))
            .await?;
        let joined = connection
            .next_event()
            .await?
            .ok_or_else(|| "signaling closed while joining room".to_owned())?;
        if !matches!(joined, ServerEvent::Joined { .. }) {
            return Err(format!("room join rejected: {joined:?}"));
        }

        Ok((connection, joined))
    }

    pub async fn send(&mut self, value: Value) -> Result<(), String> {
        self.socket
            .send(Message::Text(value.to_string().into()))
            .await
            .map_err(|error| format!("signaling send failed: {error}"))
    }

    pub async fn next_event(&mut self) -> Result<Option<ServerEvent>, String> {
        loop {
            let Some(frame) = self.socket.next().await else {
                return Ok(None);
            };
            match frame.map_err(|error| format!("signaling receive failed: {error}"))? {
                Message::Text(text) => {
                    let value: Value = serde_json::from_str(&text)
                        .map_err(|error| format!("invalid signaling JSON: {error}"))?;
                    return parse_event(value).map(Some);
                }
                Message::Close(_) => return Ok(None),
                Message::Ping(payload) => {
                    self.socket
                        .send(Message::Pong(payload))
                        .await
                        .map_err(|error| format!("signaling pong failed: {error}"))?;
                }
                Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {}
            }
        }
    }
}

fn parse_event(value: Value) -> Result<ServerEvent, String> {
    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "signaling event has no type".to_owned())?;

    match kind {
        "authenticated" => Ok(ServerEvent::Authenticated {
            user: required_string(&value, "user")?,
        }),
        "joined" => Ok(ServerEvent::Joined {
            user: required_string(&value, "user")?,
            members: parse_members(&value)?,
            media_key: value
                .get("media_key")
                .map(|_| required_string(&value, "media_key"))
                .transpose()?,
        }),
        "members" => Ok(ServerEvent::Members {
            members: parse_members(&value)?,
        }),
        "peer_joined" => Ok(ServerEvent::PeerJoined {
            user: required_string(&value, "user")?,
        }),
        "peer_left" => Ok(ServerEvent::PeerLeft {
            user: required_string(&value, "user")?,
        }),
        "chat" => Ok(ServerEvent::Chat {
            from: required_string(&value, "from")?,
            content: required_string(&value, "content")?,
        }),
        "error" => Ok(ServerEvent::Error {
            reason: required_string(&value, "reason")?,
        }),
        "signal" | "media_answer" | "media_candidate" | "media_state" | "media_failed"
        | "route_prepare" | "route_commit" | "route_active" | "route_rollback" => {
            Ok(ServerEvent::Signal(value))
        }
        _ => Err(format!("unsupported signaling event: {kind}")),
    }
}

fn required_string(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("signaling event has no string field `{key}`"))
}

fn parse_members(value: &Value) -> Result<Vec<String>, String> {
    value
        .get("members")
        .and_then(Value::as_array)
        .ok_or_else(|| "signaling event has no members".to_owned())?
        .iter()
        .map(|member| {
            member
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "signaling member is not a string".to_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn members_keepalive_is_accepted() {
        assert!(matches!(
            parse_event(json!({"type": "members", "members": ["alice", "bob"]})),
            Ok(ServerEvent::Members { members }) if members == ["alice", "bob"]
        ));
    }

    #[test]
    fn chat_event_has_server_identity() {
        assert!(matches!(
            parse_event(json!({"type":"chat","from":"alice","content":"hello"})),
            Ok(ServerEvent::Chat { from, content }) if from == "alice" && content == "hello"
        ));
    }

    #[test]
    fn joined_event_accepts_room_key() {
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, [7u8; 32]);
        assert!(matches!(
            parse_event(
                json!({"type":"joined","user":"alice","members":["alice"],"media_key":encoded})
            ),
            Ok(ServerEvent::Joined {
                media_key: Some(_),
                ..
            })
        ));
        assert!(
            parse_event(json!({"type":"joined","user":"alice","members":[],"media_key":42}))
                .is_err()
        );
    }

    #[tokio::test]
    #[ignore = "requires a running BeamVoice server and two test credentials"]
    async fn two_clients_join_and_track_presence() {
        let _ = dotenvy::dotenv();
        let url = std::env::var("PULSE_TEST_SIGNAL_URL")
            .or_else(|_| std::env::var("PULSE_SIGNAL_URL"))
            .expect("configure PULSE_TEST_SIGNAL_URL or PULSE_SIGNAL_URL");
        let room = std::env::var("PULSE_TEST_ROOM")
            .or_else(|_| std::env::var("PULSE_ROOM"))
            .expect("configure PULSE_TEST_ROOM or PULSE_ROOM");
        let first_token = std::env::var("PULSE_TEST_AUTH_TOKEN_1")
            .or_else(|_| std::env::var("PULSE_AUTH_TOKEN"))
            .expect("configure PULSE_TEST_AUTH_TOKEN_1 or PULSE_AUTH_TOKEN");
        let second_token = std::env::var("PULSE_TEST_AUTH_TOKEN_2")
            .or_else(|_| std::env::var("PULSE_AUTH_TOKEN_2"))
            .expect("configure PULSE_TEST_AUTH_TOKEN_2 or PULSE_AUTH_TOKEN_2");
        let first_config = SignalingConfig {
            url: url.clone(),
            token: first_token,
            room: room.clone(),
        };
        let second_config = SignalingConfig {
            url,
            token: second_token,
            room,
        };

        let (mut first, joined) = tokio::time::timeout(
            Duration::from_secs(5),
            SignalingConnection::connect(&first_config),
        )
        .await
        .expect("first connection timed out")
        .expect("first connection failed");
        let (first_user, first_key) = match joined {
            ServerEvent::Joined {
                user, media_key, ..
            } => (user, media_key),
            _ => unreachable!(),
        };
        let (second, joined) = tokio::time::timeout(
            Duration::from_secs(5),
            SignalingConnection::connect(&second_config),
        )
        .await
        .expect("second connection timed out")
        .expect("second connection failed");
        let second_user = match joined {
            ServerEvent::Joined {
                user,
                members,
                media_key,
            } => {
                assert!(members.contains(&first_user));
                assert_eq!(media_key, first_key);
                assert!(media_key.is_some());
                user
            }
            _ => unreachable!(),
        };

        let joined_event = tokio::time::timeout(Duration::from_secs(5), first.next_event())
            .await
            .expect("peer_joined timed out")
            .expect("peer_joined failed");
        assert!(
            matches!(joined_event, Some(ServerEvent::PeerJoined { user }) if user == second_user)
        );

        first
            .send(json!({"type": "members"}))
            .await
            .expect("keepalive failed");
        let members_event = tokio::time::timeout(Duration::from_secs(5), first.next_event())
            .await
            .expect("members timed out")
            .expect("members failed");
        assert!(
            matches!(members_event, Some(ServerEvent::Members { members })
            if members.contains(&first_user) && members.contains(&second_user))
        );

        drop(second);
        let left_event = tokio::time::timeout(Duration::from_secs(5), first.next_event())
            .await
            .expect("peer_left timed out")
            .expect("peer_left failed");
        assert!(matches!(left_event, Some(ServerEvent::PeerLeft { user }) if user == second_user));
    }
}
