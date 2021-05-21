use crate::object::Object;

#[derive(Debug)]
pub struct Blob {
    data: Vec<u8>,
}

impl Blob {
    pub fn new(data: Vec<u8>) -> Self {
        Blob { data }
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
