use std::time::Duration;

use serde::Deserialize;
use tokio::process::Command;
use tokio::time::{interval, MissedTickBehavior};

#[derive(Debug)]
pub enum UpdateError {
    Install(String, String),
    CheckVersion(String, String),
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Install(package, error) => {
                write!(f, "failed to install package: {package}: {error}")
            }
            Self::CheckVersion(package, error) => {
                write!(f, "failed to check package version: {package}: {error}")
            }
        }
    }
}

impl std::error::Error for UpdateError {}

pub async fn update_packages() -> Result<(), UpdateError> {
    static PACKAGES: [&str; 1] = ["yt-dlp"];

    for package in PACKAGES {
        install_pip_package(package).await?;
    }

    // Spawn a task that updates packages automatically
    tokio::spawn(async move {
        // 1 day
        let mut interval = interval(Duration::from_secs(24 * 60 * 60));
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            interval.tick().await;
            for package in PACKAGES {
                if let Err(e) = install_pip_package(package).await {
                    eprintln!("{e:#?}");
                }
            }
        }
    });

    Ok(())
}

async fn install_pip_package(package: &str) -> Result<(), UpdateError> {
    // Check old version
    let old_version = check_pip_package_version(package).await?;

    // Update package
    let output = Command::new("pip")
        .args(["install", "--upgrade", package])
        .output()
        .await
        .map_err(|e| UpdateError::Install(package.to_owned(), e.to_string()))?;
    if !output.status.success() {
        return Err(UpdateError::Install(
            package.to_owned(),
            "command returned non-zero exit code".to_owned(),
        ));
    }

    // Check new version
    let new_version = check_pip_package_version(package).await?.ok_or_else(|| {
        UpdateError::Install(
            package.to_owned(),
            "no package version after install".to_owned(),
        )
    })?;
    if let Some(old_version) = old_version {
        if old_version != new_version {
            print_new_version(package, &new_version);
        }
    } else {
        print_new_version(package, &new_version);
    }

    Ok(())
}

async fn check_pip_package_version(package: &str) -> Result<Option<String>, UpdateError> {
    #[derive(Deserialize)]
    struct Package {
        name: String,
        version: String,
    }

    // Run and parse pip command
    let output = Command::new("pip")
        .args(["list", "--no-index", "--format=json"])
        .output()
        .await
        .map_err(|e| UpdateError::CheckVersion(package.to_owned(), e.to_string()))?;
    if !output.status.success() {
        return Err(UpdateError::CheckVersion(
            package.to_owned(),
            "command returned non-zero exit code".to_owned(),
        ));
    }
    let json_output = String::from_utf8(output.stdout)
        .map_err(|e| UpdateError::CheckVersion(package.to_owned(), e.to_string()))?;
    let parsed_output: Vec<Package> = serde_json::from_str(&json_output)
        .map_err(|e| UpdateError::CheckVersion(package.to_owned(), e.to_string()))?;

    // Find package
    let version = parsed_output
        .iter()
        .find(|p| p.name == package)
        .map(|p| p.version.to_owned());

    Ok(version)
}

fn print_new_version(package: &str, version: &str) {
    println!("Updated package {package} to {version}");
}
