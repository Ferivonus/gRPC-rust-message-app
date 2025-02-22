use chat::chat_service_client::ChatServiceClient;
use chat::ChatMessage;
use futures::StreamExt;
use std::error::Error;
use std::io::{self, Write};
use tokio_stream::wrappers::ReceiverStream;
use tonic::Request;

pub mod chat {
    tonic::include_proto!("chat");
}

fn get_input(prompt: &str) -> Result<String, Box<dyn Error>> {
    print!("{}", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut client = ChatServiceClient::connect("http://[::1]:50051").await?;

    println!("--- Sohbete Bağlanıyor ---");
    let username = get_input("Kullanıcı Adınız: ")?;
    let username_clone = username.clone(); // Clone oluştur
    let _target_user = get_input("Hedef Kullanıcı: ")?;
    println!("--------------------------");

    let (tx, rx) = tokio::sync::mpsc::channel(10);
    let response = client
        .chat_stream(Request::new(ReceiverStream::new(rx)))
        .await?;
    let mut incoming = response.into_inner();

    let handle = tokio::spawn(async move {
        // Clone'lanmış username'i kullan
        let username = username_clone;
        while let Some(message) = incoming.next().await {
            match message {
                Ok(msg) => {
                    if msg.from != username {
                        print!("\x1B[2K\r");
                        println!(
                            "[Gelen Mesaj] [{}] {}: {}",
                            msg.timestamp, msg.from, msg.message
                        );
                    }
                    print!("Mesajınız (Çıkış için 'q'): ");
                    let _ = io::stdout().flush();
                }
                Err(e) => {
                    print!("\x1B[2K\r");
                    eprintln!("Hata: {}", e);
                    print!("Mesajınız (Çıkış için 'q'): ");
                    let _ = io::stdout().flush();
                }
            }
        }
    });

    loop {
        print!("Mesajınız (Çıkış için 'q'): ");
        io::stdout().flush()?;

        let mut message = String::new();
        io::stdin().read_line(&mut message)?;
        let message = message.trim().to_string();

        if message.to_lowercase() == "q" {
            break;
        }

        let chat_message = ChatMessage {
            from: username.clone(),
            to: "all".to_string(), // Tüm kullanıcılara gönder
            message,
            timestamp: chrono::Utc::now().timestamp(),
        };

        let _ = client
            .send_message(Request::new(chat_message.clone()))
            .await?;

        tx.send(chat_message).await?;
    }

    handle.abort();
    Ok(())
}
