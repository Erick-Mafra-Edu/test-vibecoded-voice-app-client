//! Simulation service — generates fake participant speaking activity and
//! fluctuating network metrics so the UI prototype feels alive.

use rand::Rng;

use crate::state::{AppState, ChatMessage};

/// Called on every `SimTick`. Mutates state in-place.
pub fn sim_tick(state: &mut AppState) {
    state.tick = state.tick.wrapping_add(1);
    let mut rng = rand::thread_rng();

    // ── Speaking simulation ──────────────────────────────────────────────────
    for p in state.participants.iter_mut() {
        if p.is_muted || p.is_deafened {
            p.is_speaking = false;
            p.speaking_level = (p.speaking_level * 0.85).max(0.0);
            continue;
        }

        // Each participant has a random chance of starting / stopping speaking
        let roll: f32 = rng.gen();
        if p.is_speaking {
            // Vary the speaking level
            p.speaking_level = (p.speaking_level + rng.gen_range(-0.15..0.15)).clamp(0.3, 1.0);
            if roll < 0.06 {
                // 6% chance to stop speaking each tick
                p.is_speaking = false;
            }
        } else {
            if roll < 0.04 {
                // 4% chance to start speaking each tick
                p.is_speaking = true;
                p.speaking_level = rng.gen_range(0.5..0.9);
            } else {
                p.speaking_level = (p.speaking_level * 0.8).max(0.0);
            }
        }
    }

    // ── Network metrics simulation ───────────────────────────────────────────
    let base_latency: i32 = 18;
    let spike: i32 = if rng.gen_bool(0.05) {
        rng.gen_range(30..120)
    } else {
        0
    };
    let jitter = rng.gen_range(0i32..6);
    let packet_loss: f32 = if rng.gen_bool(0.03) {
        rng.gen_range(0.5..3.5)
    } else {
        0.0
    };

    state.metrics.latency_ms = (base_latency + spike + rng.gen_range(-3i32..4)).max(1) as u32;
    state.metrics.jitter_ms = jitter as u32;
    state.metrics.packet_loss_pct = packet_loss;

    // ── Occasional auto chat message ─────────────────────────────────────────
    if state.tick % 60 == 0 {
        let msgs = [
            ("Alpha", "Boa jogada!"),
            ("Bravo", "Meu ping subiu agora."),
            ("Charlie", "Vamos pela esquerda."),
            ("Delta", "Voltei."),
            ("Bravo", "Essa foi limpa."),
            ("Alpha", "Mais uma rodada?"),
        ];
        let (author, content) = msgs[rng.gen_range(0..msgs.len())];

        let ts = chrono::Local::now().format("%H:%M").to_string();
        let msg = ChatMessage {
            author: author.to_string(),
            content: content.to_string(),
            timestamp: ts,
            is_self: false,
        };

        state.chat_messages.push_back(msg);
        // Keep history bounded
        if state.chat_messages.len() > 200 {
            state.chat_messages.pop_front();
        }
    }
}
