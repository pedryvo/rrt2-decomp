//! Version string: crate version plus the git commit recorded by `build.rs`.

/// `0.1.0 (abc123def)`, printed by `rrt2 --version`.
pub const LONG: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("RRT2_GIT_COMMIT"),
    ")"
);
