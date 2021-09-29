pub(crate) fn into_io_error<E>(error: E) -> std::io::Error
where
    E: std::error::Error + Send + Sync + 'static,
{
    std::io::Error::new(std::io::ErrorKind::Other, error)
}
