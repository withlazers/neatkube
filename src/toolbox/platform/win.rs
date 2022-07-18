use crate::result::Result;
use std::ffi::OsStr;
use std::path::Path;
use std::{convert::Infallible, ffi::CStr, fs::Permissions};
use tokio::process::Command;

pub fn execve<P, SA, SE>(path: P, args: &[SA], env: &[SE]) -> Result<Infallible>
where
    P: AsRef<Path>,
    SA: AsRef<OsStr>,
    SE: AsRef<OsStr>,
{
    todo!()
}

pub fn set_exec(permission: &mut Permissions) {
    todo!()
}

pub fn arg0<S>(command: &mut Command, arg0: S)
where
    S: AsRef<OsStr>,
{
    // No Op
}
