use std::time::Duration;

use async_trait::async_trait;

use tokio::net::UnixStream;
use tokio::sync::Mutex;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::time;
use unicom_lib::arch::unix::{UnixMessage, read_message, write_message, read_init};
use unicom_lib::error::{UnicomError, UnicomErrorKind};
use unicom_lib::node::message::UnicomMessage;
use unicom_lib::node::message::request::UnicomRequest;
use unicom_lib::node::message::response::UnicomResponse;
use unicom_lib::node::{NodeConnector, NodeConfig};
use unicom_lib::node::utils::pending::PendingController;

pub struct UnixConnector{
    reader: Mutex<OwnedReadHalf>,
    writer: Mutex<OwnedWriteHalf>,
    pending: PendingController,
}

impl UnixConnector{
    pub fn new(stream: UnixStream) -> UnixConnector{
        let (reader, writer) = stream.into_split();
        UnixConnector { 
            reader: Mutex::new(reader), 
            writer: Mutex::new(writer),
            pending: PendingController::new(),
        }

    }
    async fn read_message(&self) -> Result<UnixMessage, UnicomError>{
        read_message(&mut *self.reader.lock().await).await
    }

    async fn write_message(&self, value: UnixMessage) -> Result<(), UnicomError>{    
        write_message(&mut *self.writer.lock().await, value).await
    }
    
}

#[async_trait]
impl NodeConnector for UnixConnector {
    async fn init(&self) -> Result<NodeConfig, UnicomError>{
        read_init(&mut *self.reader.lock().await).await
    }

    async fn request(&self, request: UnicomRequest, timeout: f32) -> Result<UnicomResponse, UnicomError>{
        let (id, notify) = self.pending.create().await;

        self.write_message(UnixMessage::Request{
            id,
            data: request,
        }).await?;

        if let Err(_) = time::timeout(Duration::from_secs_f32(timeout), notify.notified()).await{
            return Err(UnicomError::new(UnicomErrorKind::Timeout, "timeout"));
        }

        Ok(UnicomResponse{data: self.pending.get(id).await?})
    }

    async fn response(&self, request_id: u64, response: UnicomResponse) -> Result<(), UnicomError>{
        self.write_message(UnixMessage::Response{
            id: request_id,
            data: response.data,
        }).await?;
        Ok(())
    }

    async fn error(&self, request_id: u64, error: UnicomError) -> Result<(), UnicomError>{
        self.write_message(UnixMessage::Error { 
            id: request_id, 
            error,
        }).await?;
        Ok(())
    }

    async fn next(&self) -> Result<UnicomMessage, UnicomError>{
        loop{
            match self.read_message().await? {
                UnixMessage::Response { id, data } => self.pending.update(id, Ok(data)).await?,
                UnixMessage::Request { id, data } => return Ok(UnicomMessage::Request { id, data }),
                UnixMessage::Quit => return Ok(UnicomMessage::Quit),
                UnixMessage::Error { id, error } => self.pending.update(id, Err(error)).await?,
            };
        }
    }

    async fn quit(&self) -> Result<(), UnicomError>{
        self.write_message(UnixMessage::Quit).await
    }
}