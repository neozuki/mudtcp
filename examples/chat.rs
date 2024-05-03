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
    let mut server = Server::<LineCodec>::new(addr.as_str()).expect("Failed to create server");
    println!("Server started with address \"{addr}\"");

    loop {
        let mut msg_in: Vec<(ClientId, String)> = vec![];
        let mut msg_out: Vec<(ClientId, String)> = vec![];
        for ev in server.poll() {
            match ev {
                Event::Join(id) => {
                    println!("Event: Client joined with id {id}.");
                    msg_out.push((id, MOTD.to_owned()));
                }
                Event::Leave(id) => {
                    println!("Event: Client with id {id} left.");
                }
                Event::Receive(msg) => {
                    println!(
                        "Event: Received {} bytes from Client with id {}.",
                        msg.1.len(),
                        msg.0,
                    );
                    msg_in.push(msg);
                }
                Event::Send(_) => {}
                Event::ClientError((id, e)) => {
                    println!("Event: Client with id {id} error: {e}.");
                }
                Event::ServerError(e) => {
                    println!("Event: Server error: {e}.");
                    break;
                }
            }
        }
        server.ids_connected().for_each(|id| {
            for m in &msg_in {
                msg_out.push((id, format!("anonymous#{} says, \"{}\"", m.0, m.1)));
            }
        });
        server.enqueue_many(msg_out.into_iter());
    }
}
