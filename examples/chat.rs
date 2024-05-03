extern crate mudtcp;
use mudtcp::{codec::LineCodec, *};

const MOTD: &str = "Welcome to the chat server!";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        println!("Usage: ./chat adress portno");
        println!("Example: ./chat 127.0.0.1 8000");
        return;
    }

    let addr = format!("{}:{}", args[1], args[2]);
    let server = Server::<LineCodec>::new(addr.as_str()).expect("Failed to create server");
    println!("Server started with address \"{addr}\"");

    loop {
        for ev in server.poll() {
            match ev {
                Event::Join(id) => {
                    server.enqueue((id, MOTD.to_owned()));
                    server.enqueue_for_each(format!("anonymous#{id} has joined.").as_str());
                }
                Event::Leave(id) => {
                    server.enqueue_for_each(format!("anonymous#{id} has left.").as_str());
                }
                Event::Receive((id, msg)) => {
                    server.enqueue_for_each(format!("anonymous#{id} says, \"{msg}\"").as_str());
                }
                Event::Send(_) => {}
                Event::ClientError((id, e)) => {
                    println!("Event: Client with id {id} error: {e}.");
                    server.enqueue_for_each(format!("anonymous#{id} has left.").as_str());
                }
                Event::ServerError(e) => {
                    println!("Event: Server error: {e}.");
                    break;
                }
            }
        }
    }
}
