# PULSE — Low-Latency Voice Hub

## GitHub build and version tags

GitHub Actions checks formatting, runs the signaling unit test, builds the
Windows executable, and uploads it as a workflow artifact for each pull request
and push to `main`. After a successful build on `main`, Release Please uses
Conventional Commits (`fix:`, `feat:`, and `feat!:`) to open a release pull
request. Merging that pull request updates `Cargo.toml` and `Cargo.lock`, then
creates the matching `vX.Y.Z` tag and GitHub Release. The repository must allow
GitHub Actions to create pull requests; the workflow's `GITHUB_TOKEN` needs
write access to contents and pull requests. The local `.env` and `target/` are
ignored by Git.

A desktop voice & text chat client built with **Rust + Iced 0.14**, featuring a Lo-Fi Cyberpunk design system.

## Features (Prototype)
- 🎙 Voice channel grid with animated speaking indicators
- 💬 Text chat with message history and bubble styling
- 🎛 Floating audio dock (mute / deafen / screen share / leave)
- 📡 Live HUD with latency, jitter, and packet-loss metrics
- 🖥 Screen share mode with compact participant sidebar
- 🎚 Local microphone/speaker/display selection
- 🎧 Real local microphone capture through CPAL
- 🎭 Overlay mode for in-game use
- ⚡ Simulated participants, speaking activity, and network fluctuations

## Architecture

```
src/
├── main.rs          — entry point, window settings, font loading
├── app/
│   ├── mod.rs       — PulseApp (update / view / subscription / theme)
│   └── messages.rs  — typed Message enum (Elm pattern)
├── state/
│   └── mod.rs       — AppState, Participant, ChatMessage, NetworkMetrics
├── theme/
│   └── mod.rs       — colour palette, fonts, all widget styles
├── components/
│   ├── avatar.rs    — canvas-based coloured avatar with speaking ring
│   ├── top_bar.rs   — logo, server picker, nav, HUD
│   ├── channel_bar.rs — voice & text channel pills
│   ├── participant_card.rs — avatar card with badges and ping
│   ├── chat_panel.rs — scrollable chat, text input, bubbles
│   ├── dock.rs      — floating audio controls
│   └── hud.rs       — latency / jitter / packet-loss HUD
├── screens/
│   ├── home.rs      — main voice + chat layout
│   ├── screen_share.rs — fullscreen share + participant strip
│   ├── overlay.rs   — compact in-game overlay
│   └── settings.rs  — settings placeholder
├── media/
│   ├── audio.rs — device enumeration and non-blocking microphone capture
│   └── screen.rs — display enumeration and frame capture validation
└── services/
    └── simulation.rs — simulated speaking and network metrics
├── runtime/
│   ├── commands.rs — commands sent from the UI to the client core
│   ├── events.rs — runtime events and immutable UI snapshots
│   ├── service_handle.rs — bounded command/event channel handle
│   └── supervisor.rs — Tokio supervisor and simulation backend
```

The Iced layer communicates with the runtime through `ClientCommand` and
`ClientEvent`. The runtime owns its simulation state and publishes bounded
snapshots, so UI updates do not perform network, audio, or simulation work.
The current simulation is a replaceable backend; a signaling implementation
can later consume the same command boundary without changing widgets.

To use the BeamVoice WebSocket signaling backend in the isolated lab, set these
environment variables before starting the client:

```powershell
$env:PULSE_SIGNAL_URL = "ws://127.0.0.1:8080/ws"
$env:PULSE_AUTH_TOKEN = "<token from the lab credentials fixture>"
$env:PULSE_ROOM = "lab"
cargo run
```

Para abrir duas instâncias usando os dois usuários de teste, abra dois
terminais na pasta do cliente e passe o token diretamente na linha de comando.
Depois de `cargo build`, use o mesmo executável em ambos os terminais:

```powershell
# Terminal 1
.\target\debug\pulse.exe --token-slot 1

# Terminal 2
.\target\debug\pulse.exe --token-slot 2
```

O parâmetro `--token-slot` seleciona `PULSE_AUTH_TOKEN_1` ou
`PULSE_AUTH_TOKEN_2` depois que o `.env` é carregado. Os parâmetros `--token`,
`--room` e `--signal-url` também substituem apenas essas variáveis na instância
iniciada. Assim, as duas janelas podem usar tokens diferentes sem alterar o
`.env` nem depender da tela de configurações. Execute os comandos a partir da
pasta do cliente para que ambas as instâncias encontrem o `.env`. Também é
possível definir `PULSE_AUTH_TOKEN` separadamente em cada terminal ou usar
`--token <valor>`.

With `PULSE_SIGNAL_URL` and `PULSE_ROOM` configured, the runtime connects after
a token is provided through the environment, command line, or Settings. It
then tracks presence and sends a `members` keepalive every 30 seconds. Without
the URL or room, the simulation backend remains active and the offline notice
explains why token changes cannot connect. Tokens are never stored in the
repository or logged.

With an updated BeamVoice backend, each authenticated member receives the same
random room key on join. Voice, chat, and screen sharing start without entering
a phrase. The backend knows this key, so this is server-managed media encryption,
not strict end-to-end encryption. Use `wss://` outside localhost because plain
WebSocket exposes the token and room key.

For an older backend without room-key negotiation, the legacy fallback requires
the same room passphrase in each client before starting it:

```powershell
$env:PULSE_E2E_PASSPHRASE = "a shared phrase of at least 16 bytes"
```

Each legacy room derives a different key. Share the fallback phrase outside
Pulse; anyone who knows it can read that room's media and chat.
For connections across NAT, configure `PULSE_ICE_URL` (`stun:`, `turn:`, or
`turns:`), and when using TURN set `PULSE_ICE_USERNAME` and
`PULSE_ICE_CREDENTIAL`. The server uses the corresponding `BEAMVOICE_ICE_*`
variables. A TURN service and reachable relay ports are required separately.
The WebSocket URL remains `ws:` or `wss:` and the room can be a catalog channel
such as `lab/general` once an administrator creates it through BeamVoice's
`/api/servers` endpoints.

No modo conectado, a interface mostra apenas a sala configurada por `PULSE_ROOM`
e os participantes informados pelo servidor. O chat da sala é efêmero e cifrado;
o HUD usa estatísticas WebRTC de RTT, jitter e perda. Em Settings,
`Audio WebRTC conectado` e `Áudio remoto recebido`
ajudam a distinguir presença no WebSocket do transporte de voz. Se duas janelas
forem abertas no mesmo computador, use fones de ouvido para evitar microfonia.
Após alterações no código, feche ambas e execute `cargo build --locked` antes
de iniciá-las novamente.

The client loads `.env` automatically when present. To diagnose the local
Docker endpoint without revealing the token, run:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\validate-backend.ps1
```

The validator checks the configured URL, container IP, published port, TCP
reachability, `/health`, authentication, and room join. The BeamVoice process must actually
be running and expose port `8080`; a toolchain container that only runs
`sleep` is not a signaling server.

## Signaling integration test

With BeamVoice running and two test credentials available in `.env`, run:

```powershell
cargo test --lib -- --ignored --nocapture
```

The test connects two clients, checks room presence, requests `members`, and
checks departure. To target another server without changing `.env`, set
`PULSE_TEST_SIGNAL_URL`, `PULSE_TEST_ROOM`, `PULSE_TEST_AUTH_TOKEN_1`, and
`PULSE_TEST_AUTH_TOKEN_2` in the test process environment. Each test variable
falls back to its corresponding `PULSE_` variable (`PULSE_AUTH_TOKEN_2` for the
second token). The server must allow both credentials into the same room.

The Settings screen includes backend authentication controls for local testing.
`PULSE_AUTH_TOKEN_1` and `PULSE_AUTH_TOKEN_2` are loaded as selectable options,
and a token can also be pasted manually. Applying a token closes the current
WebSocket and reconnects with the new credential. The selector labels tokens
without displaying their values. The offline banner stays visible while the
client uses demonstration data or waits for the backend; authentication errors
appear in Settings. Keep `.env` local and never
commit real or test credentials.

## Building

```bash
# Debug build
cargo run

# Optimised release build
cargo build --release
./target/release/pulse
```

## Design Tokens

| Token        | Value     | Use                          |
|--------------|-----------|------------------------------|
| BG_BASE      | `#090E18` | Window background            |
| BG_SURFACE   | `#151D2C` | Cards, panels                |
| BG_ELEVATED  | `#1C2638` | Dock, hover states           |
| CYAN         | `#00D9F5` | Primary accent, speaking     |
| AMBER        | `#FFB547` | Secondary accent, screen share |
| SUCCESS      | `#32CD63` | Good connection              |
| DANGER       | `#EF4444` | Muted, poor connection       |

## Local media validation

The settings screen enumerates microphones, speakers, and displays. Selecting a
microphone starts a CPAL capture stream on a dedicated thread; the Iced callback
never performs device I/O. Selecting a display is used by the screen-share
command, which captures one frame to validate the OS capture path.

The same checks can be run without opening the UI:

```powershell
cargo run -- --validate-media
```

This validates local capture only. The signaling integration test does not
verify WebRTC/Opus audio, TURN traversal, or screen transmission; check the two
audio indicators in Settings while both clients are connected.

## Roadmap
- [ ] Validate WebRTC / Opus audio across separate computers and NAT
- [x] Send encrypted JPEG screen frames over a WebRTC data channel
- [x] Server/channel CRUD via REST API
- [x] Shared passphrase encryption for voice, chat, and screen frames
