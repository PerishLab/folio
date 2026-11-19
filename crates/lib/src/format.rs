use crate::Error;
use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::Path;

const MARK: &[u8] = b"folio:1\n";

pub(crate) fn write(root: &Path) -> Result<(), Error> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join("format"))?;
    file.write_all(MARK)?;
    file.sync_all()?;
    #[cfg(unix)]
    std::fs::File::open(root)?.sync_all()?;
    Ok(())
}

pub(crate) fn read(root: &Path) -> Result<(), Error> {
    match std::fs::read(root.join("format")) {
        Ok(mark) if mark == MARK => Ok(()),
        Ok(_) => Err(Error::Corrupt),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            Err(Error::Invalid("missing Folio data format".into()))
        }
        Err(error) => Err(error.into()),
    }
}
