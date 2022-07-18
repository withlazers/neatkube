use os_str_bytes::OsStrBytes;
use std::{
    convert::Infallible,
    ffi::{CStr, CString, OsStr},
    fs::Permissions,
    os::unix::prelude::PermissionsExt,
    path::Path,
};
use tokio::process::Command;

use crate::result::Result;

pub fn execve<P, SA, SE>(path: P, args: &[SA], env: &[SE]) -> Result<Infallible>
where
    P: AsRef<Path>,
    SA: AsRef<OsStr>,
    SE: AsRef<OsStr>,
{
    let path = CString::new(path.as_ref().to_raw_bytes()).unwrap();
    let args = args
        .iter()
        .map(|s| CString::new(s.as_ref().to_raw_bytes()).unwrap())
        .collect::<Vec<_>>();
    let env = env
        .iter()
        .map(|s| CString::new(s.as_ref().to_raw_bytes()).unwrap())
        .collect::<Vec<_>>();
    Ok(nix::unistd::execve(&path, &args, &env)?)
}

pub fn set_exec(permission: &mut Permissions) {
    permission.set_mode(0o755)
}

pub fn arg0<S>(command: &mut Command, arg0: S)
where
    S: AsRef<OsStr>,
{
    command.arg0(arg0);
}
