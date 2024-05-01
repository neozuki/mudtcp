pub mod codec;
use std::{io, net, vec};
use thiserror::Error;

pub use codec::Codec;
pub type ClientId = usize;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("failed to bind server to address `{addr:?}`: {e:?}")]
    Bind { addr: String, e: io::Error },
    #[error("error polling listener socket: {0}")]
    Listener(io::Error),
    #[error("error accepting new client: {0}")]
    Accept(io::Error),
    #[error("error reading from client#{id:?}: {error:?}")]
    Read { id: ClientId, error: io::Error },
    #[error("error writing to client#{id:?}: {error:?}")]
    Write { id: ClientId, error: io::Error },
    #[error("id `{0}` not found")]
    NotFound(ClientId),
}

#[derive(Debug)]
pub enum Event {
    Join(ClientId),
    Leave(ClientId),
    Receive((ClientId, String)),
    Send(ClientId),
    ClientError((ClientId, io::Error)),
    ServerError(io::Error),
}

#[derive(Debug)]
pub struct Server<C: Codec> {
    listener: net::TcpListener,
    codecs: Vec<C>,
    events: Vec<Event>,
    message_queue: Vec<(ClientId, String)>,
    last_id: ClientId,
}

impl<C: Codec> Server<C> {
    pub fn new(addr: &str) -> Result<Self, ServerError> {
        match net::TcpListener::bind(addr) {
            Ok(listener) => match listener.set_nonblocking(true) {
                Ok(_) => Ok(Self {
                    listener,
                    codecs: vec![],
                    events: vec![],
                    message_queue: vec![],
                    last_id: 0,
                }),
                Err(e) => Err(ServerError::Listener(e)),
            },
            Err(e) => Err(ServerError::Bind {
                addr: addr.to_owned(),
                e,
            }),
        }
    }

    pub fn poll(&mut self) -> vec::Drain<Event> {
        self.codecs.retain(|c| c.is_open());

        self.send_messages();
        self.poll_clients();
        self.poll_listener();

        self.events.drain(..)
    }

    fn poll_listener(&mut self) {
        let mut streams: Vec<net::TcpStream> = vec![];
        for stream in self.listener.incoming() {
            match stream {
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    /* no pending connections */
                    break;
                }
                Err(e) => {
                    self.events.push(Event::ServerError(e));
                    break;
                }
                Ok(stream) => {
                    streams.push(stream);
                }
            }
        }
        for stream in streams.into_iter() {
            match self.try_accept(stream) {
                Ok(codec) => {
                    self.events.push(Event::Join(codec.id()));
                    self.codecs.push(codec);
                }
                Err(e) => {
                    self.events.push(Event::ServerError(e));
                }
            }
        }
    }

    fn try_accept(&mut self, stream: net::TcpStream) -> io::Result<C> {
        stream.set_nonblocking(true)?;
        let id = self.next_id();
        let codec = C::new(id, stream)?;
        Ok(codec)
    }

    fn poll_clients(&mut self) {
        for codec in self.codecs.iter_mut() {
            match codec.read() {
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => { /* no pending data */ }
                Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    /* assume client left voluntarily */
                    codec.shutdown();
                    self.events.push(Event::Leave(codec.id()));
                }
                Ok(data) => {
                    let m = (codec.id(), data);
                    self.events.push(Event::Receive(m));
                }
                Err(e) => {
                    codec.shutdown();
                    self.events.push(Event::ClientError((codec.id(), e)));
                }
            }
        }
    }

    pub fn enqueue(&mut self, msg: (ClientId, String)) {
        self.message_queue.push(msg);
    }

    pub fn enqueue_many(&mut self, msgs: impl Iterator<Item = (ClientId, String)>) {
        msgs.for_each(|msg| self.message_queue.push(msg));
    }

    fn send_messages(&mut self) {
        for (id, msg) in self.message_queue.drain(..) {
            if let Some(codec) = self.codecs.iter_mut().find(|c| c.id() == id) {
                if !codec.is_open() {
                    continue; /* should generate some 'SendFail' event */
                }
                if let Err(e) = codec.write(msg.as_str()) {
                    codec.shutdown();
                    self.events.push(Event::ClientError((codec.id(), e)));
                } else {
                    self.events.push(Event::Send(codec.id()));
                }
            } else {
                println!(
                    "Unable to send message to Client#{}: No match for Message ClientId",
                    id
                );
            }
        }
    }

    fn next_id(&mut self) -> ClientId {
        if ClientId::MAX == self.last_id {
            panic!("Server ran out of ClientIds");
        }
        self.last_id += 1;
        self.last_id
    }

    pub fn kick(&mut self, id: ClientId) -> Result<(), ServerError> {
        if let Some(codec) = self.codecs.iter_mut().find(|c| c.id() == id) {
            codec.shutdown();
            Ok(())
        } else {
            Err(ServerError::NotFound(id))
        }
    }
}
