pub mod codec;
use std::{cell, io, net, vec};
use thiserror::Error;

pub use codec::Codec;
pub type ClientId = usize;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("failed to bind server to address `{addr:?}`: {e:?}")]
    Bind {
        addr: String,
        #[source]
        e: io::Error,
    },
    #[error("error polling listener socket: {0}")]
    Listener(#[source] io::Error),
    #[error("error accepting new client: {0}")]
    Accept(#[source] io::Error),
    #[error("error reading from client#{id:?}: {error:?}")]
    Read {
        id: ClientId,
        #[source]
        error: io::Error,
    },
    #[error("error writing to client#{id:?}: {error:?}")]
    Write {
        id: ClientId,
        #[source]
        error: io::Error,
    },
    #[error("id `{0}` not found")]
    IdNotFound(ClientId),
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
    codecs: cell::RefCell<Vec<C>>,
    events: cell::RefCell<Vec<Event>>,
    message_queue: cell::RefCell<Vec<(ClientId, String)>>,
    last_id: cell::RefCell<ClientId>,
}

impl<C: Codec> Server<C> {
    pub fn new(addr: &str) -> Result<Self, ServerError> {
        match net::TcpListener::bind(addr) {
            Ok(listener) => match listener.set_nonblocking(true) {
                Ok(_) => Ok(Self {
                    listener,
                    codecs: cell::RefCell::new(vec![]),
                    events: cell::RefCell::new(vec![]),
                    message_queue: cell::RefCell::new(vec![]),
                    last_id: cell::RefCell::new(0),
                }),
                Err(e) => Err(ServerError::Listener(e)),
            },
            Err(e) => Err(ServerError::Bind {
                addr: addr.to_owned(),
                e,
            }),
        }
    }

    pub fn poll(&self) -> Vec<Event> {
        self.codecs.borrow_mut().retain(|c| c.is_open());

        self.send_messages();
        self.poll_clients();
        self.poll_listener();

        self.events.borrow_mut().drain(..).collect()
    }

    fn poll_listener(&self) {
        for stream in self.listener.incoming() {
            match stream {
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    /* no pending connections */
                    break;
                }
                Err(e) => {
                    self.events.borrow_mut().push(Event::ServerError(e));
                    break;
                }
                Ok(stream) => match self.try_accept(stream) {
                    Ok(codec) => {
                        self.events.borrow_mut().push(Event::Join(codec.id()));
                        self.codecs.borrow_mut().push(codec);
                    }
                    Err(e) => {
                        self.events.borrow_mut().push(Event::ServerError(e));
                    }
                },
            }
        }
    }

    fn try_accept(&self, stream: net::TcpStream) -> io::Result<C> {
        stream.set_nonblocking(true)?;
        let id = self.next_id();
        let codec = C::new(id, stream)?;
        Ok(codec)
    }

    fn poll_clients(&self) {
        for codec in self.codecs.borrow_mut().iter_mut() {
            match codec.read() {
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => { /* no pending data */ }
                Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    /* assume client left voluntarily */
                    codec.shutdown();
                    self.events.borrow_mut().push(Event::Leave(codec.id()));
                }
                Ok(data) => {
                    let m = (codec.id(), data);
                    self.events.borrow_mut().push(Event::Receive(m));
                }
                Err(e) => {
                    codec.shutdown();
                    self.events
                        .borrow_mut()
                        .push(Event::ClientError((codec.id(), e)));
                }
            }
        }
    }

    pub fn enqueue(&self, msg: (ClientId, String)) {
        self.message_queue.borrow_mut().push(msg);
    }

    pub fn enqueue_many(&self, msgs: impl Iterator<Item = (ClientId, String)>) {
        msgs.for_each(|msg| self.message_queue.borrow_mut().push(msg));
    }

    fn send_messages(&self) {
        for (id, msg) in self.message_queue.borrow_mut().drain(..) {
            if let Some(codec) = self.codecs.borrow_mut().iter_mut().find(|c| c.id() == id) {
                if !codec.is_open() {
                    continue; /* should generate some 'SendFail' event */
                }
                if let Err(e) = codec.write(msg.as_str()) {
                    codec.shutdown();
                    self.events
                        .borrow_mut()
                        .push(Event::ClientError((codec.id(), e)));
                } else {
                    self.events.borrow_mut().push(Event::Send(codec.id()));
                }
            } else {
                println!(
                    "Unable to send message to Client#{}: No match for Message ClientId",
                    id
                ); /* should generate some 'SendFail' event */
            }
        }
    }

    fn next_id(&self) -> ClientId {
        let mut last = self.last_id.borrow_mut();
        if ClientId::MAX == *last {
            panic!("Server ran out of ClientIds");
        }
        *last += 1;
        *last
    }

    pub fn kick(&self, id: ClientId) -> Result<(), ServerError> {
        if let Some(codec) = self.codecs.borrow_mut().iter_mut().find(|c| c.id() == id) {
            codec.shutdown();
            Ok(())
        } else {
            Err(ServerError::IdNotFound(id))
        }
    }

    pub fn ids(&self) -> Vec<(ClientId, bool)> {
        self.codecs
            .borrow()
            .iter()
            .map(|c| (c.id(), c.is_open()))
            .collect()
    }

    pub fn ids_connected(&self) -> Vec<ClientId> {
        self.codecs
            .borrow()
            .iter()
            .filter(|&c| c.is_open())
            .map(|c| c.id())
            .collect()
    }

    pub fn ids_disconnected(&self) -> Vec<ClientId> {
        self.codecs
            .borrow()
            .iter()
            .filter(|&c| !c.is_open())
            .map(|c| c.id())
            .collect()
    }

    pub fn enqueue_for_each(&self, msg: &str) {
        for id in self.ids_connected() {
            self.enqueue((id, msg.to_owned()));
        }
    }
}
