use std::path::PathBuf;

use crate::error::AppResult;
use crate::security::files::ensure_private_dir;
use crate::security::vault::VaultPaths;

pub struct AppState {
    pub app_data: PathBuf,
    pub paths: VaultPaths,
}

impl AppState {
    pub fn new(app_data: PathBuf) -> AppResult<Self> {
        ensure_private_dir(&app_data)?;
        let paths = VaultPaths::new(&app_data);
        Ok(Self { app_data, paths })
    }
}
