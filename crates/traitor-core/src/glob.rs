use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu)]
#[snafu(display("{} has an invalid glob in paths, {glob:?}: {source}", path.display()))]
pub struct GlobError {
    path: PathBuf,
    glob: String,
    source: globset::Error,
}

pub(crate) fn compile_globs(path: &Path, globs: &[String]) -> Result<GlobSet, GlobError> {
    globs
        .iter()
        .try_fold(GlobSetBuilder::new(), |mut builder, glob| {
            builder.add(
                GlobBuilder::new(glob)
                    .literal_separator(true)
                    .build()
                    .context(GlobSnafu { path, glob })?,
            );
            Ok(builder)
        })?
        .build()
        .with_context(|_| GlobSnafu {
            path,
            glob: globs.join(", "),
        })
}
