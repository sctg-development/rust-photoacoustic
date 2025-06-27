// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Build and version information module
//!
//! This module provides access to build-time information including Git commit hashes,
//! compilation date, and other metadata useful for maintenance and debugging.

use chrono::{DateTime, Utc};

/// Build information structure containing all relevant metadata
#[derive(Debug, Clone)]
pub struct BuildInfo {
    /// Cargo package version
    pub version: &'static str,
    /// Short Git commit hash (7 characters)
    pub git_commit_short: &'static str,
    /// Full Git commit hash (40 characters)
    pub git_commit_full: &'static str,
    /// Git commit date in ISO format
    pub git_commit_date: &'static str,
    /// Build timestamp (when the binary was compiled)
    pub build_timestamp: &'static str,
    /// Rust compiler version used for build
    pub rustc_version: &'static str,
    /// Target triple (architecture and OS)
    pub target_triple: &'static str,
    /// Build profile (debug/release)
    pub profile: &'static str,
}

impl BuildInfo {
    /// Get the current build information
    pub fn get() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            git_commit_short: env!("GIT_COMMIT_HASH_SHORT"),
            git_commit_full: env!("GIT_COMMIT_HASH_FULL"),
            git_commit_date: env!("GIT_COMMIT_DATE"),
            build_timestamp: env!("BUILD_TIMESTAMP"),
            rustc_version: env!("BUILD_RUSTC_VERSION"),
            target_triple: env!("BUILD_TARGET"),
            profile: if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
        }
    }

    /// Format build info as a single version string
    /// Example: "1.0.0-a1b2c3d (2025-06-27 14:30:00 UTC)"
    pub fn version_string(&self) -> String {
        format!(
            "{}-{} ({})",
            self.version, self.git_commit_short, self.git_commit_date
        )
    }

    /// Format complete build information for debug output
    pub fn full_info(&self) -> String {
        format!(
            "Version: {}\n\
            Hash: {} ({})\n\
            Build Time: {}\n\
            Rust Version: {}\n\
            Target: {}\n\
            Profile: {}",
            self.version,
            self.git_commit_short,
            self.git_commit_date,
            self.build_timestamp,
            self.rustc_version,
            self.target_triple,
            self.profile
        )
    }

    /// Check if this is a dirty build (uncommitted changes)
    pub fn is_dirty_build(&self) -> bool {
        self.git_commit_short.ends_with("-dirty") || self.git_commit_full.ends_with("-dirty")
    }

    /// Get just the clean commit hash without dirty marker
    pub fn clean_commit_hash(&self) -> &str {
        if self.is_dirty_build() {
            &self.git_commit_short[..self.git_commit_short.len() - 6] // Remove "-dirty"
        } else {
            self.git_commit_short
        }
    }

    /// Parse the Git commit date as a DateTime
    pub fn commit_datetime(&self) -> Result<DateTime<Utc>, chrono::ParseError> {
        DateTime::parse_from_str(self.git_commit_date, "%Y-%m-%d %H:%M:%S %z")
            .map(|dt| dt.with_timezone(&Utc))
    }
}

impl std::fmt::Display for BuildInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.version_string())
    }
}

/// Print version information to stdout
/// This is useful for implementing --version flags
pub fn print_version_info() {
    let build_info = BuildInfo::get();
    println!("{}", build_info.version_string());
}

/// Print full build information to stdout
/// This is useful for implementing --build-info flags
pub fn print_build_info() {
    let build_info = BuildInfo::get();
    println!("{}", build_info.full_info());
}

/// Get version hash for maintenance purposes
/// Returns the full Git commit hash, which is what you need for precise maintenance
pub fn get_version_hash() -> &'static str {
    BuildInfo::get().git_commit_full
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_info_creation() {
        let info = BuildInfo::get();

        // Basic checks that the info is populated
        assert!(!info.version.is_empty());
        assert!(!info.git_commit_short.is_empty());
        assert!(!info.git_commit_full.is_empty());

        // Check that short hash is actually shorter than full hash
        assert!(info.git_commit_short.len() <= info.git_commit_full.len());
    }

    #[test]
    fn test_version_string_format() {
        let info = BuildInfo::get();
        let version_str = info.version_string();

        // Should contain version and hash
        assert!(version_str.contains(info.version));
        assert!(version_str.contains(info.clean_commit_hash()));
    }

    #[test]
    fn test_dirty_build_detection() {
        let info = BuildInfo::get();

        // Just test that the function doesn't panic
        let _is_dirty = info.is_dirty_build();
        let _clean_hash = info.clean_commit_hash();
    }
}
