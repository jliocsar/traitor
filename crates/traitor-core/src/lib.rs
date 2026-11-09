mod climb;
mod git;
mod glob;

pub mod case;
pub mod check;
pub mod config;
pub mod configured_repository;
pub mod init;
pub mod judge;
pub mod question;
pub mod stop_hook;
pub mod store;
pub mod trait_definition;
pub mod verdict;

pub use climb::Snippet;
pub use git::GitError;
pub use glob::GlobError;

#[cfg(test)]
mod test_support;
