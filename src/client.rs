use chat::chat_service_client::ChatServiceClient;
use chat::ChatMessage;
use chrono::Utc;
use futures::StreamExt;
use std::io;
use std::{error::Error, io::Write};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_stream::wrappers::ReceiverStream;
use tonic::Request;

pub mod chat {
    tonic::include_proto!("chat");
}

async fn get_input(prompt: &str) -> Result<String, Box<dyn Error>> {
    print!("{}", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    let mut reader = BufReader::new(tokio::io::stdin());
    reader.read_line(&mut input).await?;

    Ok(input.trim().to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut client = ChatServiceClient::connect("http://127.0.0.1:50051").await?;

    println!("--- Chat Login ---");
    let username = get_input("Username: ").await?;
    let target = get_input("Target user: ").await?;
    println!("------------------");

    let (tx, rx) = tokio::sync::mpsc::channel(10);
    let mut request_stream = ReceiverStream::new(rx);

    // Send registration message
    let registration_message = ChatMessage {
        from: username.clone(),
        to: target.clone(),
        message: "REGISTER".to_string(), // Or an empty string
        timestamp: Utc::now().timestamp(),
    };
    tx.send(registration_message).await?;

    let response = client.chat_stream(Request::new(request_stream)).await?;
    let mut incoming = response.into_inner();

    let handle = tokio::spawn(async move {
        while let Some(message) = incoming.next().await {
            match message {
                Ok(msg) => {
                    print!("\x1B[2K\r");
                    println!("[{}] {}: {}", msg.timestamp, msg.from, msg.message);
                    print!("Your message (q to quit): ");
                    let _ = io::stdout().flush();
                }
                Err(e) => {
                    print!("\x1B[2K\r");
                    eprintln!("Error: {}", e);
                    print!("Your message (q to quit): ");
                    let _ = io::stdout().flush();
                }
            }
        }
    });

    loop {
        print!("Your message (q to quit): ");
        let message = get_input("").await?;

        if message.to_lowercase() == "q" {
            break;
        }

        let chat_message = ChatMessage {
            from: username.clone(),
            to: target.clone(),
            message,
            timestamp: Utc::now().timestamp(),
        };

        client
            .send_message(Request::new(chat_message.clone()))
            .await?;
        tx.send(chat_message).await?;
    }

    handle.abort();
    Ok(())
}
