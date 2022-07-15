use sha1::digest::Update;
use sha1::{Digest, Sha1};

pub trait Object {
    fn r#type(&self) -> &str;

    fn bytes(&self) -> Vec<u8>;

    fn oid(&self) -> String {
        let hash = Sha1::new().chain(&self.content()).finalize();
        format!("{:x}", hash)
    }

    fn content(&self) -> Vec<u8> {
        let bytes = &mut self.bytes();
        let mut content = format!("{} {}\0", &self.r#type(), bytes.len()).into_bytes();
        content.append(bytes);
        content
    }
}
