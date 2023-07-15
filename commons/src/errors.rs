use std::error::Error;

pub type AsyncError = Box<dyn Error + Send + Sync>;
