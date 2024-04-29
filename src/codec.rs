mod linecodec;
pub use linecodec::LineCodec;
use crate::ClientId;

pub trait Codec: Sized {
    fn new(id: ClientId, stream: std::net::TcpStream) -> std::io::Result<Self>;
    fn read(&mut self) -> std::io::Result<String>;
    fn write(&mut self, message: &str) -> std::io::Result<()>;
    fn shutdown(&mut self);
    fn is_open(&self) -> bool;
    fn id(&self) -> ClientId;
}
