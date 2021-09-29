use std::path::{Component, Path, PathBuf};

pub(crate) fn into_io_error<E>(error: E) -> std::io::Error
where
    E: std::error::Error + Send + Sync + 'static,
{
    std::io::Error::new(std::io::ErrorKind::Other, error)
}

pub fn join(base: &mut PathBuf, dir: &Path) {
    for c in dir.components() {
        match c {
            Component::Prefix(_) => todo!(),
            Component::RootDir => {
                *base = dir.to_owned();
                break;
            }
            Component::CurDir => { /* ignore */ }
            Component::ParentDir => {
                base.pop();
            }
            Component::Normal(v) => {
                base.push(v);
            }
        }
    }
}
