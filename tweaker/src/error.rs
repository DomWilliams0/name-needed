pub use failure::{Backtrace, Context, Fail, ResultExt};

#[derive(Debug)]
pub struct TweakerError {
    inner: Context<TweakerErrorKind>,
}

pub(crate) type TweakerResult<T> = Result<T, TweakerError>;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum TweakerErrorKind {
    #[fail(display = "Could not connect to local tweaker server on port {}", port)]
    SocketConnection { port: u16 },

    #[fail(display = "Could not read from socket")]
    SocketIo,

    #[fail(display = "Invalid JSON received")]
    InvalidJson,

    #[fail(display = "Unsupported JSON type received for value")]
    UnsupportedJsonType,

    #[fail(display = "Bad JSON root type received")]
    BadRootJsonType,
}

// boilerplate

impl Fail for TweakerError {
    fn cause(&self) -> Option<&dyn Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl std::fmt::Display for TweakerError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.inner, f)
    }
}

impl TweakerError {
    pub fn kind(&self) -> TweakerErrorKind {
        *self.inner.get_context()
    }
}

impl From<TweakerErrorKind> for TweakerError {
    fn from(kind: TweakerErrorKind) -> TweakerError {
        TweakerError {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<TweakerErrorKind>> for TweakerError {
    fn from(inner: Context<TweakerErrorKind>) -> TweakerError {
        TweakerError { inner }
    }
}
