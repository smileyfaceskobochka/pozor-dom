use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use pozor_dom_shared::{connection, messages};
use std::env;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <server_url>", args[0]);
        eprintln!("Example: {} ws://localhost:8081", args[0]);
        eprintln!("Example: {} ws://localhost:8082", args[0]);
        std::process::exit(1);
    }

    let server_url = &args[1];
    println!("Pozor-dom Client starting...");
    println!("Connecting to: {}", server_url);
    println!("Type messages to send to server. Press Ctrl+C to exit.");

    match connect_async(server_url).await {
        Ok((ws_stream, _)) => {
            println!("Connected to server successfully!");
            let (mut write, mut read) = ws_stream.split();

            // Send hello message to server
            let hello_msg = messages::create_welcome_message("Client");
            if let Err(e) = write.send(hello_msg).await {
                eprintln!("Failed to send hello message: {}", e);
                return;
            }

            let mut rx = connection::setup_stdin_channel().await.expect("Failed to setup stdin channel");

            // Main loop to handle both WebSocket messages and stdin input
            loop {
                tokio::select! {
                    // Handle incoming WebSocket messages
                    message = read.next() => {
                        match message {
                            Some(Ok(Message::Text(text))) => {
                                println!("Received: {}", text);

                                // Only echo back if this is not a response message (to prevent infinite loop)
                                if !messages::is_response_message(&text) {
                                    // Echo back with client prefix
                                    let echo_msg = messages::create_echo_message("Client", &text);
                                    if let Err(e) = write.send(echo_msg).await {
                                        eprintln!("Failed to send echo: {}", e);
                                        break;
                                    }
                                }
                            }
                            Some(Ok(Message::Close(_))) => {
                                println!("Connection closed by server");
                                break;
                            }
                            Some(Ok(other)) => {
                                println!("Received non-text message: {:?}", other);
                            }
                            Some(Err(e)) => {
                                eprintln!("Error receiving message: {}", e);
                                break;
                            }
                            None => break,
                        }
                    }

                    // Handle stdin input
                    input = rx.recv() => {
                        match input {
                            Some(text) => {
                                println!("Sending: {}", text);
                                if let Err(e) = write.send(Message::Text(text.into())).await {
                                    eprintln!("Failed to send message: {}", e);
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to connect to server: {}", e);
            eprintln!("Make sure the server is running on {}", server_url);
        }
    }
}
