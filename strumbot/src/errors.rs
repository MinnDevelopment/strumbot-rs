use std::fmt::{self, Display, Formatter};

use thiserror::Error;

#[derive(Error, Debug)]
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
