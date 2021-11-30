use std::{io, path::PathBuf};

/// Gets WGPU trace path for this application.
#[cfg(feature = "debug-wgpu-trace")]
pub async fn get_trace_path(
    typename: impl AsRef<str>,
    start_date: bool,
) -> Result<Option<PathBuf>, io::Error> {
    use crate::util::get_start_date;
    use chrono::Local;
    use std::borrow::Cow;
    use tokio::fs::create_dir_all;

    let date = if start_date {
        Cow::Borrowed(get_start_date())
    } else {
        Cow::Owned(Local::now())
    };

    let path = PathBuf::from(format!(
        "logs/log-{}-{}.trace",
        date.format("%Y-%m-%d_%H-%M-%S").to_string(),
        typename.as_ref()
    ));

    create_dir_all(&path).await?;

    Ok(Some(path))
}

/// Gets WGPU trace path for this application.
#[cfg(not(feature = "debug-wgpu-trace"))]
pub async fn get_trace_path(
    _typename: impl AsRef<str>,
    _start_date: bool,
) -> Result<Option<PathBuf>, io::Error> {
    Ok(None)
}
