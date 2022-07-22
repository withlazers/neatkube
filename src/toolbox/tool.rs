use std::{
    collections::HashMap,
    env,
    ffi::{CString, OsStr, OsString},
    iter,
    ops::{Deref, DerefMut},
    os::unix::prelude::{OsStrExt, PermissionsExt},
    path::PathBuf,
    process::Stdio,
    str::FromStr,
};

use crate::download::Downloader;
use crate::error::Error;
use crate::result::Result;
use clap::Arg;
use nix::unistd::execve;
use os_str_bytes::OsStrBytes;
use serde::Deserialize;
use tokio::{
    fs, fs::File, io::AsyncReadExt, io::AsyncWriteExt, process::Command,
    sync::Mutex,
};
use tokio_stream::StreamExt;

use super::{
    upstream::{Upstream, UpstreamDefinition},
    Toolbox,
};
use dewey::VersionCmp;

static ARGS_NAME: &str = "args";

#[derive(Debug, Clone)]
pub enum VersionRef {
    Latest,
    Local,
    Specific(String),
}

impl VersionRef {}

impl FromStr for VersionRef {
    type Err = core::convert::Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(s.into())
    }
}

impl From<String> for VersionRef {
    fn from(s: String) -> Self {
        match s.as_str() {
            "latest" => VersionRef::Latest,
            "local" => VersionRef::Local,
            _ => VersionRef::Specific(s),
        }
    }
}

impl From<&str> for VersionRef {
    fn from(s: &str) -> Self {
        Self::from(s.to_string())
    }
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ToolDefinition {
    name: String,
    description: String,
    upstream: UpstreamDefinition,
    #[serde(default)]
    extract_command: String,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    os_map: HashMap<String, String>,
    #[serde(default)]
    arch_map: HashMap<String, String>,
}

impl ToolDefinition {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn description(&self) -> &str {
        &self.description
    }
    fn replace(&self, input: &str, version: &str) -> Result<String> {
        Ok(minitmpl::minitmpl_fn(input, |x| match x {
            "name" => Some(self.name.as_str()),
            "version" => Some(version),
            "os" => Some(self.os()),
            "arch" => Some(self.arch()),
            "stripped_version" => version.strip_prefix('v').or(Some(version)),
            _ => None,
        })?)
    }

    pub fn os(&self) -> &str {
        let default = match std::env::consts::OS {
            "macos" => "darwin",
            x => x,
        };

        self.os_map
            .get(default)
            .map(String::as_str)
            .unwrap_or(default)
    }

    pub fn arch(&self) -> &str {
        let default = match std::env::consts::ARCH {
            "x86_64" => "amd64",
            "x86" => "386",
            "aarch64" => "arm64",
            x => x,
        };

        self.arch_map
            .get(default)
            .map(String::as_str)
            .unwrap_or(default)
    }

    fn upstream<'a>(&'a self) -> Box<dyn Upstream + 'a> {
        match &self.upstream {
            UpstreamDefinition::GithubRelease(upstream) => Box::new(upstream),
            UpstreamDefinition::Simple(upstream) => Box::new(upstream),
        }
    }

    pub fn extract_command(&self, version: &str) -> Result<Vec<String>> {
        self.extract_command
            .split(' ')
            .filter(|x| !x.is_empty())
            .map(|x| self.replace(x, version))
            .collect()
    }

    pub fn package_url(&self, version: &str) -> Result<String> {
        self.replace(&self.upstream().package_url(), version)
    }
}

pub struct Tool<'a> {
    pub toolbox: &'a Toolbox,
    pub definition: &'a ToolDefinition,
    pub version: Mutex<Vec<VersionRef>>,
}

impl<'a> Tool<'a> {
    pub fn new(definition: &'a ToolDefinition, toolbox: &'a Toolbox) -> Self {
        Self {
            toolbox,
            definition,
            version: Mutex::new(vec![VersionRef::Local, VersionRef::Latest]),
        }
    }
    pub fn new_with_version(
        definition: &'a ToolDefinition,
        toolbox: &'a Toolbox,
        version_refs: Vec<VersionRef>,
    ) -> Self {
        Self {
            definition,
            toolbox,
            version: Mutex::new(version_refs),
        }
    }

    pub async fn resolve_version(&self) -> Result<String> {
        let mut guard = self.version.lock().await;
        let version_refs = guard.deref();

        let mut result = Err("No version found".into());
        for version_ref in version_refs {
            let version = match version_ref {
                VersionRef::Latest => Some(self.find_latest_version().await?),
                VersionRef::Local => self.find_local_version().await?,
                VersionRef::Specific(version) => {
                    return Ok(version.to_string());
                }
            };
            if let Some(version) = version {
                result = Ok(version);
                break;
            }
        }

        match result {
            Ok(version) => {
                let version_ref = guard.deref_mut();
                *version_ref = vec![VersionRef::Specific(version.clone())];
                Ok(version)
            }
            e => e,
        }
    }
    pub fn name(&self) -> &str {
        &self.definition.name
    }

    pub fn description(&self) -> &str {
        &self.definition.description
    }

    pub async fn exec_dir_path(&self) -> Result<PathBuf> {
        let version = self.resolve_version().await?;
        let path = self
            .toolbox
            .exec_dir_path()?
            .join(self.name())
            .join(version);

        Ok(path)
    }

    async fn exec_path(&self) -> Result<PathBuf> {
        let name = &self.name();
        Ok(self.exec_dir_path().await?.join(name))
    }

    pub async fn find_local_version(&self) -> Result<Option<String>> {
        let all_versions = self.find_local_versions().await?;

        Ok(all_versions.into_iter().last())
    }

    pub async fn run<'b, I, S>(&self, args: I) -> Result<()>
    where
        S: AsRef<[u8]>,
        I: IntoIterator<Item = S>,
    {
        if !self.is_installed().await? {
            self.install(false).await?;
        }

        self.exec(args).await
    }

    async fn exec<'b, I, S>(&self, args: I) -> Result<()>
    where
        S: AsRef<[u8]>,
        I: IntoIterator<Item = S>,
    {
        let tool_name = self.name();
        let bin = self.exec_path().await?;
        let bin = CString::new(bin.to_raw_bytes()).unwrap();
        let args = args.into_iter().map(|x| CString::new(x.as_ref()).unwrap());
        let exec_args = iter::once(CString::new(tool_name).unwrap())
            .chain(args)
            .collect::<Vec<_>>();
        execve(&bin, &exec_args, &self.get_exec_env().await?)?;
        Ok(())
    }

    pub async fn command<I, S>(&self, args: I) -> Result<Command>
    where
        S: AsRef<OsStr>,
        I: IntoIterator<Item = S>,
    {
        if !self.is_installed().await? {
            self.install(false).await?;
        }

        let tool_name = self.name();
        let bin = self.exec_path().await?;

        let mut command = Command::new(&bin);
        command.arg0(tool_name).args(args);
        Ok(command)
    }

    async fn build_path_env(&self) -> Result<OsString> {
        let mut result = OsString::new();
        for dep in self.definition.dependencies.iter() {
            let tool = self
                .toolbox
                .tool_with_version(dep, vec![VersionRef::Local])?;
            result.push(tool.exec_dir_path().await?);
            result.push(":");
        }
        result.push(self.toolbox.exec_dir_path()?);
        result.push(":");
        Ok(result)
    }

    async fn get_exec_env(&self) -> Result<Vec<CString>> {
        let bin_dir_path = self.build_path_env().await?;
        let new_env = env::vars_os()
            .into_iter()
            .map(|(name, value)| match name.to_str() {
                Some("PATH") => {
                    let mut new_value = bin_dir_path.clone();
                    new_value.push(value);
                    (name, new_value)
                }
                _ => (name, value),
            })
            .map(|(mut var, value)| {
                var.push("=");
                var.push(value);
                CString::new(var.as_bytes()).unwrap()
            })
            .collect();
        Ok(new_env)
    }

    fn downloader(&self) -> &Downloader {
        self.toolbox.downloader()
    }

    pub async fn find_latest_version(&self) -> Result<String> {
        let url = self.definition.upstream().version_url();
        let response = self
            .downloader()
            .string(&url, &format!("{} check latest", self.name()))
            .await?;
        let latest_version = self
            .definition
            .upstream()
            .parse_version_from_response(&response)?;
        Ok(latest_version)
    }

    pub async fn find_local_versions(&self) -> Result<Vec<String>> {
        let exec_dir = self.toolbox.exec_dir_path()?.join(self.name());
        let mut versions = vec![];
        if !exec_dir.exists() {
            return Ok(vec![]);
        }

        let mut dir = fs::read_dir(&exec_dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let exec_path = entry.path().join(self.name());
            if !exec_path.exists() {
                continue;
            }
            let version_name = entry.file_name().to_str().map(String::from);

            if let Some(file_name) = version_name {
                versions.push(file_name.to_string());
            }
        }
        versions.sort_by(|a, b| a.ver_cmp(b).unwrap());
        Ok(versions)
    }

    pub async fn install(&self, force: bool) -> Result<bool> {
        let mut result = self.real_install(force).await?;
        for dep in self.definition.dependencies.iter() {
            let tool = self.toolbox.tool_with_version(
                dep,
                vec![VersionRef::Local, VersionRef::Latest],
            )?;
            let dep_result = tool.real_install(force).await?;
            result = result || dep_result;
        }
        Ok(result)
        //let (main, deps) = join!(
        //    self.real_install(force),
        //    join_all(self.definition.dependencies.iter().map(|dep| async {
        //        let tool = self.toolbox.tool(dep)?;
        //        Ok::<_, Error>(tool.real_install(false).await?)
        //    }))
        //);
        //deps.into_iter().find(Result::is_err).unwrap_or(main)
    }

    async fn real_install(&self, force: bool) -> Result<bool> {
        if self.is_installed().await? && !force {
            return Ok(false);
        }
        let version = self.resolve_version().await?;
        let bin_path = self.exec_path().await?;
        let temp_bin_path = bin_path.with_extension("part");
        let url = self.definition.package_url(&version)?;

        let mut stream = self
            .downloader()
            .stream(&url, &format!("{}-{}", self.name(), version))
            .await?;

        fs::create_dir_all(&self.exec_dir_path().await?).await?;
        let mut file = File::create(&temp_bin_path).await?;

        let extract_command = self.definition.extract_command(&version)?;
        if extract_command.is_empty() {
            while let Some(chunk) = stream.next().await {
                file.write_all(&chunk?).await?;
            }
        } else {
            let mut process = Command::new(&extract_command[0])
                .args(&extract_command[1..])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()?;
            let (reader, writer) = tokio::join!(
                async {
                    let mut stdin = process.stdin.take().unwrap();
                    while let Some(chunk) = stream.next().await {
                        stdin.write_all(&chunk?).await?;
                    }

                    Ok::<_, Error>(())
                },
                async {
                    let mut stdout = process.stdout.take().unwrap();
                    let mut buffer = [0u8; 1024];
                    while let Ok(n) = stdout.read(&mut buffer).await {
                        if n == 0 {
                            break;
                        }
                        file.write_all(&buffer[..n]).await?;
                    }

                    Ok::<_, Error>(())
                }
            );
            let result = process.wait().await?;
            if !result.success() {
                return Err(format!(
                    "Failed to extract binary: {:?}",
                    result.code()
                )
                .into());
            }

            reader?;
            writer?;
        }
        let mut permission = file.metadata().await?.permissions();
        permission.set_mode(0o755);
        fs::set_permissions(&temp_bin_path, permission).await?;
        fs::rename(&temp_bin_path, &bin_path).await?;

        Ok(true)
    }

    pub async fn is_installed(&self) -> Result<bool> {
        match self.resolve_version().await {
            Ok(v) => v,
            Err(_) => return Ok(false),
        };
        Ok(self.exec_path().await?.exists())
    }

    pub fn get_args(matches: &clap::ArgMatches) -> clap::Values<'_> {
        matches
            .values_of(ARGS_NAME)
            .or_else(|| matches.values_of(""))
            .unwrap_or_default()
    }

    pub fn subcommand(&self) -> clap::Command<'a> {
        let command = clap::Command::new(self.name())
            .disable_help_flag(true)
            .disable_help_subcommand(true)
            .allow_hyphen_values(true)
            .arg(Arg::with_name(ARGS_NAME).multiple(true))
            .aliases(
                &self
                    .definition
                    .aliases
                    .iter()
                    .map(|x| x.as_str())
                    .collect::<Vec<_>>(),
            )
            .about(&*self.definition.description);

        command
    }

    pub async fn remove(self) -> Result<()> {
        let bin_path = self.exec_path().await?;
        if bin_path.exists() {
            fs::remove_file(&bin_path).await?;
        }
        Ok(())
    }
}
