use crate::util::poll_unpin;
use rfd::{AsyncFileDialog, FileHandle};
use std::{future::Future, task::Poll};
use tokio::{
    runtime::Handle,
};

/// Asynchronously runs a file dialog.
pub struct FileDialogWrapper {
    handle: Handle,
    dialog: Option<Box<dyn Future<Output = Option<FileHandle>> + Send + Unpin + 'static>>,
}

impl FileDialogWrapper {
    /// Creates a new FileDialogWrapper with the given runtime handle.
    pub fn new(handle: Handle) -> FileDialogWrapper {
        FileDialogWrapper {
            handle,
            dialog: None,
        }
    }

    /// Opens a save file dialog.
    pub fn save_file(&mut self, dialog: AsyncFileDialog) -> Result<(), OpenError> {
        if self.dialog.is_some() {
            return Err(OpenError::AlreadyOpen);
        }

        self.dialog = Some(Box::new(dialog.save_file()));

        Ok(())
    }

    /// Polls this wrapper to see if the dialog has been closed.
    ///
    /// Returns:
    /// * Err(..): when an error occurred while polling.
    /// * Ok(None): if nothing happened.
    /// * Ok(Some(None)): if the dialog was closed and no file was selected.
    /// * Ok(Some(Some(..))): if the dialog was closed and a file was selected.
    pub fn poll(&mut self) -> Option<Option<FileHandle>>{
        if self.dialog.is_some() {
            if let Poll::Ready(res) = poll_unpin(&self.handle, self.dialog.as_mut().unwrap()) {
                self.dialog = None;

                return Some(res);
            }
        }

        return None;
    }
}

#[derive(Debug, Error)]
pub enum OpenError {
    #[error("This dialog is already open")]
    AlreadyOpen,
}
