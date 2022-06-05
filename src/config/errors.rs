use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Debug)]
pub enum InitError {
    NoGuilds,
    TooManyGuilds,
}

impl Display for InitError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            InitError::NoGuilds => write!(f, "No guilds found"),
            InitError::TooManyGuilds => write!(f, "Too many guilds found"),
        }
    }
}

impl Error for InitError {}
