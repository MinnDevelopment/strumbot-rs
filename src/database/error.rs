use std::fmt::Display;

#[derive(Debug)]
pub enum DatabaseError {
    Io(std::io::Error),
    Serde(serde_json::Error),
}

impl Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseError::Io(e) => write!(f, "IO error: {}", e),
            DatabaseError::Serde(e) => write!(f, "Serde error: {}", e),
        }
    }
}

impl std::error::Error for DatabaseError {}

impl From<std::io::Error> for DatabaseError {
    fn from(e: std::io::Error) -> Self {
        DatabaseError::Io(e)
    }
}

impl From<serde_json::Error> for DatabaseError {
    fn from(e: serde_json::Error) -> Self {
        DatabaseError::Serde(e)
    }
}
