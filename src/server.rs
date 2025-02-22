use chat::chat_service_server::{ChatService, ChatServiceServer};
use chat::{ChatMessage, ChatResponse};
use chrono::Utc;
use futures::Stream;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio_stream::StreamExt;
use tonic::{transport::Server, Request, Response, Status};

pub mod chat {
    tonic::include_proto!("chat");
}

#[derive(Debug)]
struct UserChannel {
    tx: broadcast::Sender<ChatMessage>,
}

#[derive(Default)]
struct SharedState {
    users: Mutex<HashMap<String, UserChannel>>,
}

pub struct MyChatService {
    state: Arc<SharedState>,
}

impl MyChatService {
    fn new() -> Self {
        Self {
            state: Arc::new(SharedState::default()),
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
        let users = self.state.users.lock().await;

        if let Some(receiver) = users.get(&message.to) {
            println!("[SERVER] Forwarding message to: {}", message.to);
            receiver
                .tx
                .send(message.clone())
                .map_err(|_| Status::not_found("User not online"))?;
        } else {
            println!("[SERVER] User not found: {}", message.to);
        }

        Ok(Response::new(ChatResponse {
            success: true,
            info: "Message delivered".into(),
        }))
    }

    type ChatStreamStream =
        std::pin::Pin<Box<dyn Stream<Item = Result<ChatMessage, Status>> + Send + 'static>>;

    async fn chat_stream(
        &self,
        request: Request<tonic::Streaming<ChatMessage>>,
    ) -> Result<Response<Self::ChatStreamStream>, Status> {
        let mut stream = request.into_inner();

        // İlk mesajı işle
        let first_msg = match stream.next().await {
            Some(Ok(msg)) => {
                println!("[SERVER] Registration message received: {:?}", msg);
                msg
            }
            Some(Err(e)) => return Err(e.into()),
            None => return Err(Status::invalid_argument("Initial message required")),
        };

        let username = first_msg.from.clone();
        let (tx, mut rx) = broadcast::channel(10);

        println!("[SERVER] New user registered: {}", username);
        self.state
            .users
            .lock()
            .await
            .insert(username.clone(), UserChannel { tx: tx.clone() });

        let state = self.state.clone();
        tokio::spawn(async move {
            while let Some(Ok(mut msg)) = stream.next().await {
                let mut users = state.users.lock().await;
                msg.timestamp = Utc::now().timestamp(); // Zaman damgasını güncelle
                if let Some(receiver) = users.get_mut(&msg.to) {
                    println!("[SERVER] Routing message to: {}", msg.to);
                    let _ = receiver.tx.send(msg);
                }
            }

            // Cleanup on disconnect
            state.users.lock().await.remove(&username);
            println!("[SERVER] User disconnected: {}", username);
        });

        let output_stream = async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(msg) => yield Ok(msg),
                    Err(_) => break,
                }
            }
        };

        Ok(Response::new(Box::pin(output_stream)))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:50051".parse()?;
    let service = MyChatService::new();

    println!("Server running on: {}", addr);

    Server::builder()
        .add_service(ChatServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
