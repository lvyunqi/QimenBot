use async_trait::async_trait;
use qimen_error::Result;
use qimen_protocol_core::{IncomingPacket, OutgoingPacket, TransportMode};

#[derive(Debug, Clone)]
pub struct SessionMeta {
    pub session_id: String,
    pub transport_mode: TransportMode,
    pub bot_instance: String,
}

#[async_trait]
pub trait TransportSession: Send + Sync {
    fn meta(&self) -> &SessionMeta;
    async fn send(&self, packet: OutgoingPacket) -> Result<()>;
}

#[async_trait]
pub trait TransportClient: Send + Sync {
    async fn connect(&self) -> Result<Box<dyn TransportSession>>;
}

#[async_trait]
pub trait TransportServer: Send + Sync {
    async fn start(&self) -> Result<()>;
}

#[async_trait]
pub trait PacketStream: Send + Sync {
    async fn next_packet(&mut self) -> Result<Option<IncomingPacket>>;
}
