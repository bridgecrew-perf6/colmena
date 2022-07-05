//! Nix Flake utilities.

use std::convert::AsRef;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Deserialize;
use tokio::process::Command;

use super::{NixCheck, ColmenaError, ColmenaResult};

/// A Nix Flake.
#[derive(Debug)]
pub struct Flake {
    /// The flake metadata.
    metadata: FlakeMetadata,

    /// The directory the flake lives in, if it's a local flake.
    local_dir: Option<PathBuf>,
}

/// A `nix flake metadata --json` invocation.
#[derive(Deserialize, Debug)]
struct FlakeMetadata {
    /// The resolved URL of the flake.
    #[serde(rename = "resolvedUrl")]
    resolved_url: String,

    /// The locked URL of the flake.
    url: String,
}

impl Flake {
    /// Creates a flake from the given directory.
    ///
    /// This will try to retrieve the resolved URL of the local flake
    /// in the specified directory.
    pub async fn from_dir<P: AsRef<Path>>(dir: P) -> ColmenaResult<Self> {
        NixCheck::require_flake_support().await?;

        let flake = dir.as_ref().as_os_str().to_str()
            .expect("Flake directory path contains non-UTF-8 characters");

        let metadata = FlakeMetadata::resolve(flake).await?;

        Ok(Self {
            metadata,
            local_dir: Some(dir.as_ref().to_owned()),
        })
    }

    /// Creates a flake from a Flake URI.
    pub async fn from_uri(uri: String) -> ColmenaResult<Self> {
        NixCheck::require_flake_support().await?;

        let metadata = FlakeMetadata::resolve(&uri).await?;

        Ok(Self {
            metadata,
            local_dir: None,
        })
    }

    /// Returns the URI.
    pub fn uri(&self) -> &str {
        &self.metadata.resolved_url
    }

    /// Returns the locked URI.
    ///
    /// Note that the URI will not be locked if the git workspace
    /// is dirty.
    pub fn locked_uri(&self) -> &str {
        &self.metadata.url
    }

    /// Returns the local directory, if it exists.
    pub fn local_dir(&self) -> Option<&Path> {
        self.local_dir.as_deref()
    }
}

impl FlakeMetadata {
    /// Resolves a flake.
    async fn resolve(flake: &str) -> ColmenaResult<Self> {
        let child = Command::new("nix")
            .args(&["flake", "metadata", "--json"])
            .args(&["--experimental-features", "nix-command flakes"])
            .arg(flake)
            .stdout(Stdio::piped())
            .spawn()?;

        let output = child.wait_with_output().await?;

        if !output.status.success() {
            return Err(output.status.into());
        }

        serde_json::from_slice::<FlakeMetadata>(&output.stdout)
            .map_err(|_| {
                let output = String::from_utf8_lossy(&output.stdout).to_string();
                ColmenaError::BadOutput { output }
            })
    }
}
