#[derive(Debug)]
pub enum Error {
    Unauthorized,
    Denied,
    Invalid(String),
    Missing,
    Conflict,
    Unavailable,
    Corrupt,
    Document(keel_document::Error),
    Engine(keel::adapt::Error),
    Protocol(radicula_document::Error),
    Patch(radicula_document::Fault),
    Io(std::io::Error),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unauthorized => "unauthorized",
            Self::Denied => "denied",
            Self::Invalid(_) | Self::Protocol(_) | Self::Patch(_) => "invalid",
            Self::Missing => "missing",
            Self::Conflict => "conflict",
            Self::Document(keel_document::Error::Conflict { .. }) => "conflict",
            Self::Document(keel_document::Error::Engine(_)) => "denied",
            Self::Engine(keel::adapt::Error::Adapt(_)) => "denied",
            _ => "unavailable",
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(self.code())
    }
}

impl std::error::Error for Error {}

impl From<keel_document::Error> for Error {
    fn from(error: keel_document::Error) -> Self {
        Self::Document(error)
    }
}

impl From<keel::adapt::Error> for Error {
    fn from(error: keel::adapt::Error) -> Self {
        Self::Engine(error)
    }
}

impl From<radicula_document::Error> for Error {
    fn from(error: radicula_document::Error) -> Self {
        Self::Protocol(error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
