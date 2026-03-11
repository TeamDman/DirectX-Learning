pub type MyResult<T, E = MyReport> = core::result::Result<T, E>;

pub struct MyReport {
    inner: eyre::Report,
}
impl From<eyre::Report> for MyReport {
    fn from(report: eyre::Report) -> Self {
        Self { inner: report }
    }
}
impl std::fmt::Display for MyReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl std::fmt::Debug for MyReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl From<windows::core::Error> for MyReport {
    fn from(error: windows::core::Error) -> Self {
        Self {
            inner: eyre::Report::new(WrappedWindowsError::from(error)),
        }
    }
}

pub struct WrappedWindowsError {
    inner: windows::core::Error,
}
impl From<windows::core::Error> for WrappedWindowsError {
    fn from(error: windows::core::Error) -> Self {
        Self { inner: error }
    }
}

impl std::error::Error for WrappedWindowsError {}
impl std::fmt::Display for WrappedWindowsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl std::fmt::Debug for WrappedWindowsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}
