use chat::chat_service_server::{ChatService, ChatServiceServer};
use chat::{ChatMessage, ChatResponse};
use chrono::Utc;
use futures::Stream;
use std::sync::Arc;
use tokio::sync::broadcast;
use tonic::{transport::Server, Request, Response, Status};

pub mod chat {
    tonic::include_proto!("chat");
}

#[derive(Debug)]
struct SharedState {
    tx: broadcast::Sender<ChatMessage>,
}

pub struct MyChatService {
    state: Arc<SharedState>,
}

impl MyChatService {
    fn new() -> Self {
        let (tx, _) = broadcast::channel(10);
        Self {
            state: Arc::new(SharedState { tx }),
        }
    }
}

#[tonic::async_trait]
impl ChatService for MyChatService {
    async fn send_message(
        &self,
        request: Request<ChatMessage>,
    ) -> Result<Response<ChatResponse>, Status> {
        let message = request.into_inner();
        println!(
            "[{}] {} -> {}: {}",
            message.timestamp, message.from, message.to, message.message
        );

        Ok(Response::new(ChatResponse {
            success: true,
            info: format!("Mesaj iletildi: {}", message.timestamp),
        }))
    }

    type ChatStreamStream =
        std::pin::Pin<Box<dyn Stream<Item = Result<ChatMessage, Status>> + Send + 'static>>;

    async fn chat_stream(
        &self,
        request: Request<tonic::Streaming<ChatMessage>>,
    ) -> Result<Response<Self::ChatStreamStream>, Status> {
        let mut stream = request.into_inner();
        let mut rx = self.state.tx.subscribe();

        // StreamExt trait'i için gerekli import
        use tokio_stream::StreamExt as _;

        let tx = self.state.tx.clone();
        tokio::spawn(async move {
            while let Some(Ok(mut msg)) = stream.next().await {
                msg.timestamp = Utc::now().timestamp();
                let _ = tx.send(msg);
            }
        });

        let output_stream = async_stream::try_stream! {
            loop {
                let msg = rx.recv().await
                    .map_err(|e| Status::internal(format!("Yayın hatası: {}", e)))?;
                yield msg;
            }
        };

        Ok(Response::new(Box::pin(output_stream)))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let service = MyChatService::new();

    println!("Sunucu çalışıyor: {}", addr);

    Server::builder()
        .add_service(ChatServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
