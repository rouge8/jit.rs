use crate::database::object::Object;
use crate::database::ParsedObject;
use sha1::{Digest, Sha1};

#[derive(Debug)]
pub struct Blob {
    pub data: Vec<u8>,
    oid: Option<String>,
}

impl Blob {
    pub fn new(data: Vec<u8>) -> Self {
        Blob { data, oid: None }
    }

    pub fn parse(data: &[u8], oid: &str) -> ParsedObject {
        ParsedObject::Blob(Blob {
            data: data.to_vec(),
            oid: Some(oid.to_string()),
        })
    }
}

impl Object for Blob {
    fn r#type(&self) -> &str {
        "blob"
    }

    fn oid(&self) -> String {
        match &self.oid {
            Some(oid) => oid.to_string(),
            None => {
                let hash = Sha1::new().chain(&self.content()).finalize();
                format!("{:x}", hash)
            }
        }
    }

    fn bytes(&self) -> Vec<u8> {
        self.data.clone()
    }
}
