//! Handle used by the UI to communicate with the runtime.

use std::{
    hash::{Hash, Hasher},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::BroadcastStream;

use super::{ClientCommand, ClientEvent};

const COMMAND_CAPACITY: usize = 64;
const EVENT_CAPACITY: usize = 128;
static NEXT_RUNTIME_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub struct RuntimeHandle {
    commands: mpsc::Sender<ClientCommand>,
    events: Arc<broadcast::Sender<ClientEvent>>,
    id: u64,
}

impl Hash for RuntimeHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl RuntimeHandle {
    pub(crate) fn new(
        commands: mpsc::Sender<ClientCommand>,
        events: broadcast::Sender<ClientEvent>,
    ) -> Self {
        Self {
            commands,
            events: Arc::new(events),
            id: NEXT_RUNTIME_ID.fetch_add(1, Ordering::Relaxed),
        }
    }

    pub(crate) fn command_channel() -> (mpsc::Sender<ClientCommand>, mpsc::Receiver<ClientCommand>)
    {
        mpsc::channel(COMMAND_CAPACITY)
    }

    pub(crate) fn event_channel() -> broadcast::Sender<ClientEvent> {
        let (sender, _) = broadcast::channel(EVENT_CAPACITY);
        sender
    }

    pub fn send(&self, command: ClientCommand) {
        if let Err(error) = self.commands.try_send(command) {
            log::warn!("client runtime command was not accepted: {error}");
        }
    }

    pub fn events(&self) -> BroadcastStream<ClientEvent> {
        BroadcastStream::new(self.events.subscribe())
    }
}
