#[cfg(target_family = "unix")]
mod unix;
#[cfg(target_family = "unix")]
pub use self::unix::*;

#[cfg(target_family = "windows")]
mod win;
#[cfg(target_family = "windows")]
pub use self::win::*;
