use crate::{ClientId, codec::Codec};
use std::io::{BufRead, Write};
use std::str::FromStr;

#[derive(Debug)]
pub struct LineCodec {
    reader: std::io::BufReader<std::net::TcpStream>,
    writer: std::io::LineWriter<std::net::TcpStream>,
    open: bool,
    id: ClientId,
}

impl Codec for LineCodec {
    fn new(id: ClientId, stream: std::net::TcpStream) -> std::io::Result<Self> {
        let writer = std::io::LineWriter::new(stream.try_clone()?);
        let reader = std::io::BufReader::new(stream);
        Ok(Self { reader, writer, open: true, id })
    }

    fn read(&mut self) -> std::io::Result<String> {
        if !self.open {
            return Err(std::io::ErrorKind::NotConnected.into());
        }

        let mut buf = String::new();
        match self.reader.read_line(&mut buf) {
            Ok(bytes_read) if bytes_read > 0 => {
                let mut start = 0_usize;
                let mut end = bytes_read - 1;
                for c in buf.chars() {
                    if !c.is_whitespace() {
                        break;
                    }
                    start += 1;
                }
                for c in buf.chars().rev() {
                    if !c.is_whitespace() {
                        break;
                    }
                    if 0 == end {
                        return Err(std::io::ErrorKind::WouldBlock.into());
                    }
                    end -= 1;
                }
                let line = String::from_str(&buf.as_str()[start..=end]).unwrap();
                Ok(line)
            },
            Ok(_) => {
                Err(std::io::ErrorKind::UnexpectedEof.into())
            },
            Err(e) => {
                Err(e)
            },
        }
    }

    fn write(&mut self, msg: &str) -> std::io::Result<()> {
        if !self.open {
            return Err(std::io::ErrorKind::NotConnected.into());
        }
        self.writer.write_all(msg.as_bytes())?;
        self.writer.write_all(&[b'\n'])?;
        Ok(())
    }

    fn shutdown(&mut self) {
        self.open = false;
    }

    fn is_open(&self) -> bool {
        self.open
    }

    fn id(&self) -> ClientId {
        self.id
    }
}