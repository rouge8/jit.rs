use crate::database::object::Object;
use crate::database::ParsedObject;

#[derive(Debug)]
pub struct Blob {
    data: Vec<u8>,
}

impl Blob {
    pub fn new(data: Vec<u8>) -> Self {
        Blob { data }
    }

    pub fn parse(data: &[u8]) -> ParsedObject {
        ParsedObject::Blob(Blob::new(data.to_vec()))
    }
}

impl Object for Blob {
    fn r#type(&self) -> &str {
        "blob"
    }

    fn bytes(&self) -> Vec<u8> {
        self.data.clone()
    }
}
