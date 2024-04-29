pub mod codec;
pub use codec::Codec;
pub type ClientId = usize;

#[derive(Debug)]
pub enum Event {
    Join(ClientId),
    Leave(ClientId),
    Receive((ClientId, String)),
    Send(ClientId),
    ClientError((ClientId, std::io::Error)),
    ServerError(std::io::Error),
}

impl Clone for Event {
    fn clone(&self) -> Self {
        match self {
            Event::Join(id) => {
                Event::Join(*id)
            },
            Event::Leave(id) => {
                Event::Leave(*id)
            },
            Event::Receive(msg) => {
                Event::Receive(msg.clone())
            },
            Event::Send(id) => {
                Event::Send(*id)
            },
            Event::ClientError((id, e)) => {
                Event::ClientError((*id, e.kind().into()))
            },
            Event::ServerError(e) => {
                Event::ServerError(e.kind().into())
            },
        }
    }
}

#[derive(Debug)]
pub struct Server<C: Codec> {
    listener: std::net::TcpListener,
    codecs: Vec<C>,
    events: Vec<Event>,
    message_queue: Vec<(ClientId, String)>,
    last_id: ClientId,
}

impl<C: Codec> Server<C> {
    pub fn new(addr: &str) -> std::io::Result<Self> {
        let listener = std::net::TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            codecs: vec![],
            events: vec![],
            message_queue: vec![],
            last_id: 0,
        })
    }

    pub fn poll(&mut self) -> &[Event] {
        self.prune_clients();
        self.events.clear();

        self.send_messages();
        self.poll_clients();
        self.poll_listener();
        
        &self.events[..]
    }

    fn poll_listener(&mut self) {
        let mut streams: Vec<std::net::TcpStream> = vec![];
        for stream in self.listener.incoming() {
            match stream {
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    /* no pending connections */
                    break;
                },
                Err(e) => {
                    self.events.push(Event::ServerError(e));
                    break;
                },
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
                },
                Err(e) => {
                    self.events.push(Event::ServerError(e));
                }
            }
        }
    }

    fn try_accept(&mut self, stream: std::net::TcpStream) -> std::io::Result<C> {
        stream.set_nonblocking(true)?;
        let id = self.next_id();
        let codec = C::new(id, stream)?;
        Ok(codec)
    }

    fn poll_clients(&mut self) {
        for codec in self.codecs.iter_mut() {
            match codec.read() {
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    /* no pending data */
                },
                Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    /* assume client left voluntarily */
                    codec.shutdown();
                    self.events.push(Event::Leave(codec.id()));
                }
                Ok(data) => {
                    let m = (codec.id(), data);
                    self.events.push(Event::Receive(m));
                },
                Err(e) => {
                    codec.shutdown();
                    self.events.push(Event::ClientError((codec.id(), e)));
                },
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
                if let Err(e) = codec.write(msg.as_str()) {
                    codec.shutdown();
                    self.events.push(Event::ClientError((codec.id(), e)));
                }
                else {
                    self.events.push(Event::Send(codec.id()));
                }
            }
            else {
                println!("Unable to send message to Client#{}: No match for Message ClientId",
                    id);
            }
        }
    }

    fn prune_clients(&mut self) {
        //Clear dead connections; ie closed codecs
        if self.codecs.is_empty() {
            return;
        }

        let mut index = 0_usize;
        let mut end = self.codecs.len();
        while index < end {
            match self.codecs[index].is_open() {
                true => {
                    index += 1;
                },
                false => {
                    self.codecs.remove(index);
                    end -= 1;
                },
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
}
