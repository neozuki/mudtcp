mod usertoken;
pub use usertoken::UserToken;
use super::ClientId;
pub trait Token: Sized {
    fn new(id: ClientId) -> Self;
    fn id(&self) -> ClientId;
}
