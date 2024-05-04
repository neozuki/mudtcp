use crate::{codec::Codec, ClientId};
use std::str::FromStr;
use std::{
    io::{self, BufRead, Write},
    net,
};

/// Line-based codec that assumes [ASCII](https://www.ascii-code.com/ASCII) encoding.
///
/// [C0 control codes](https://wikipedia.org/wiki/C0_and_C1_control_codes#C0_controls) and sequences are
/// stripped out and ignored. Leading / trailing whitespace is removed.
/// For example (underscores representing whitespace): `__fo^X^Ao_b^[[1;5Aar_` becomes `foo_bar`.
#[derive(Debug)]
pub struct LineCodec {
    reader: io::BufReader<net::TcpStream>,
    writer: io::LineWriter<net::TcpStream>,
    open: bool,
    id: ClientId,
}

impl Codec for LineCodec {
    fn new(id: ClientId, stream: net::TcpStream) -> io::Result<Self> {
        let writer = io::LineWriter::new(stream.try_clone()?);
        let reader = io::BufReader::new(stream);
        Ok(Self {
            reader,
            writer,
            open: true,
            id,
        })
    }

    fn read(&mut self) -> io::Result<String> {
        if !self.open {
            return Err(io::ErrorKind::NotConnected.into());
        }

        let mut buf = String::new();
        match self.reader.read_line(&mut buf) {
            Ok(bytes_read) if bytes_read > 0 => {
                let mut line = String::new();
                let mut index = 0_usize;
                let mut sequence = false;
                while index < bytes_read {
                    let c = buf.chars().nth(index).unwrap();
                    let cb = c as u8;
                    match sequence {
                        false => {
                            if c.is_ascii_control() {
                                if cb == 0x1b {
                                    sequence = true;
                                    index += 1;
                                }
                            } else {
                                line.push(c);
                            }
                        }
                        true => {
                            if (0x20..=0x2f).contains(&cb) || (0x30..=0x3f).contains(&cb) {
                                /* sequence intermediate bytes */
                            } else {
                                /* technically, final byte should be within (0x40 ..= 0x7F) */
                                sequence = false;
                            }
                        }
                    }
                    index += 1;
                }
                line = String::from_str(line.trim()).unwrap();
                if line.is_empty() {
                    Err(io::ErrorKind::WouldBlock.into())
                } else {
                    Ok(line)
                }
            }
            Ok(_) => Err(io::ErrorKind::UnexpectedEof.into()),
            Err(e) => Err(e),
        }
    }

    fn write(&mut self, msg: &str) -> io::Result<()> {
        if !self.open {
            return Err(io::ErrorKind::NotConnected.into());
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
