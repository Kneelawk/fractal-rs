use anyhow::Context;
use liquid_core::{error::CloneableError, partials::PartialSource};
use std::{
    borrow::Cow,
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

/// Loads templates from files.
#[derive(Clone, Debug)]
pub struct FilePartialSource {
    root: PathBuf,
    names_set: HashSet<String>,
    names: Vec<String>,
}

impl FilePartialSource {
    /// Creates a new file partial source.
    ///
    /// This caches what files exist so that file existence can quickly be
    /// ascertained. But this means that if files are created or deleted, a new
    /// file partial source must be created.
    pub fn new(root: impl AsRef<Path>) -> anyhow::Result<FilePartialSource> {
        let mut names = vec![];
        walk_dir_contents(root.as_ref(), root.as_ref(), &mut names)
            .context("Error finding files for source")?;

        Ok(FilePartialSource {
            root: root.as_ref().to_path_buf(),
            names_set: names.iter().cloned().collect(),
            names,
        })
    }
}

impl PartialSource for FilePartialSource {
    fn contains(&self, name: &str) -> bool {
        self.names_set.contains(name)
    }

    fn names(&self) -> Vec<&str> {
        self.names.iter().map(|s| s.as_str()).collect()
    }

    fn try_get<'a>(&'a self, name: &str) -> Option<Cow<'static, str>> {
        let path = self.root.join(name);

        fs::read_to_string(&path).ok().map(|s| Cow::Owned(s))
    }

    fn get<'a>(&'a self, name: &str) -> liquid_core::Result<Cow<'static, str>> {
        let path = self.root.join(name);

        let str = fs::read_to_string(&path).map_err(|err| {
            liquid_core::Error::with_msg("Reading file")
                .context("file", path.to_string_lossy().to_string())
                .cause(CloneableError::new(err))
        })?;

        Ok(Cow::Owned(str))
    }
}

fn walk_dir_contents(root: &Path, dir: &Path, names: &mut Vec<String>) -> anyhow::Result<()> {
    for i in fs::read_dir(dir).with_context(|| format!("Error reading directory {:?}", dir))? {
        let entry = i.with_context(|| format!("Error reading entry from directory {:?}", dir))?;
        let path = entry.path();
        let md = fs::metadata(&path)
            .with_context(|| format!("Error reading file metadata {:?}", &path))?;
        let relative_path = pathdiff::diff_paths(&path, root).ok_or(anyhow!(
            "Unable to get relative path. Root: {:?}, current: {:?}",
            root,
            &path
        ))?;

        if md.is_dir() {
            walk_dir_contents(&root, &path, names)?;
        } else if md.is_file() {
            names.push(relative_path.to_string_lossy().to_string());
        } else {
            warn!("Encountered non-file, non-directory file: {:?}", &path);
        }
    }

    Ok(())
}
