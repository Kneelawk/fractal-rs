use crate::util::future::future_wrapper::FutureWrapper;
use rfd::{AsyncFileDialog, FileHandle};
use std::future::Future;
use tokio::runtime::Handle;

/// Asynchronously runs a file dialog.
pub struct FileDialogWrapper {
    handle: Handle,
    dialog: FutureWrapper<Box<dyn Future<Output = Option<FileHandle>> + Send + Unpin + 'static>>,
}

impl FileDialogWrapper {
    /// Creates a new FileDialogWrapper with the given runtime handle.
    pub fn new(handle: Handle) -> FileDialogWrapper {
        FileDialogWrapper {
            handle,
            dialog: Default::default(),
        }
    }

    /// Opens a save file dialog.
    pub fn save_file(&mut self, dialog: AsyncFileDialog) -> Result<(), OpenError> {
        if self.dialog.contains_future() {
            return Err(OpenError::AlreadyOpen);
        }

        self.dialog.insert(Box::new(dialog.save_file())).unwrap();

        Ok(())
    }

    /// Polls this wrapper to see if the dialog has been closed.
    ///
    /// Returns:
    /// * None: if nothing happened.
    /// * Some(None): if the dialog was closed and no file was selected.
    /// * Some(Some(..)): if the dialog was closed and a file was selected.
    pub fn poll(&mut self) -> Option<Option<FileHandle>> {
        self.dialog.poll_unpin(&self.handle)
    }
}

#[derive(Debug, Error)]
pub enum OpenError {
    #[error("This dialog is already open")]
    AlreadyOpen,
}
