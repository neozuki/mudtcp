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

    let mut ids: Vec<ClientId> = vec![];
    loop {
        let mut message_in: Vec<String> = vec![];
        let mut message_out: Vec<(ClientId, String)> = vec![];
        for ev in server.poll() {
            match ev {
                Event::Join(id) => {
                    println!("Event: Client joined with id {id}.");
                    ids.push(id);
                    message_out.push((id, MOTD.to_owned()));
                }
                Event::Leave(id) => {
                    println!("Event: Client with id {id} left.");
                    let index = ids.iter().position(|_id| *_id == id).expect("ID not found");
                    ids.remove(index);
                }
                Event::Receive(msg) => {
                    println!(
                        "Event: Received {} bytes from Client with id {}.",
                        msg.1.len(),
                        msg.0,
                    );
                    message_in.push(format!("Client[{}] says, \"{}\"", msg.0, msg.1));
                }
                Event::Send(_) => {}
                Event::ClientError((id, e)) => {
                    println!("Event: Client with id {id} error: {e}.");
                    let index = ids.iter().position(|_id| *_id == id).expect("ID not found");
                    ids.remove(index);
                }
                Event::ServerError(e) => {
                    println!("Event: Server error: {e}.");
                    break;
                }
            }
        }

        ids.iter().for_each(|id| {
            for m in message_in.iter() {
                message_out.push((*id, m.clone()));
            }
        });

        server.enqueue_many(message_out.into_iter());
    }
    //cleanup
}
