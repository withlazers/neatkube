use std::{
    env,
    ffi::{CString, OsString},
    iter,
    ops::{Deref, DerefMut},
    os::unix::prelude::{OsStrExt, PermissionsExt},
    path::PathBuf,
    process::{ExitStatus, Stdio},
    str::FromStr,
};

use crate::download::Downloader;
use crate::error::Error;
use crate::result::Result;
use clap::Arg;
use nix::unistd::execve;
use serde::Deserialize;
use tokio::{
    fs, fs::File, io::AsyncReadExt, io::AsyncWriteExt, process::Command,
    sync::Mutex,
};
use tokio_stream::StreamExt;

use super::{upstream::UpstreamDefinition, Toolbox};
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
    pub name: String,
    pub upstream: UpstreamDefinition,
    #[serde(default)]
    extract_command: String,
    pub description: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    pub alias: Option<String>,
}

impl ToolDefinition {
    fn replace(input: &str, version: &str) -> String {
        input.replace("{version}", version).replace(
            "{stripped_version}",
            version.strip_prefix("v").unwrap_or(version),
        )
    }
    pub fn extract_command(&self, version: &str) -> Vec<String> {
        self.extract_command
            .split(' ')
            .filter(|x| !x.is_empty())
            .map(|x| Self::replace(x, version))
            .collect()
    }
}

pub struct Tool<'a> {
    toolbox: &'a Toolbox,
    definition: &'a ToolDefinition,
    version: Mutex<Vec<VersionRef>>,
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

        //println!("Resolving version for {}: {:?}", self.name(), version_refs);
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

    pub async fn run(&self, args: &[String]) -> Result<()> {
        if !self.is_installed().await? {
            self.install(false).await?;
        }

        self.exec(args).await
    }

    pub async fn command(&self, args: &[String]) -> Result<Command> {
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
                .tool_with_version(&dep, vec![VersionRef::Local])?;
            result.push(tool.exec_dir_path().await?);
            result.push(":");
        }
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
            .map(|(name, value)| {
                let mut res = OsString::from(name);
                res.push("=");
                res.push(value);
                CString::new(res.as_bytes()).unwrap()
            })
            .collect();
        Ok(new_env)
    }

    pub async fn exec(&self, args: &[String]) -> Result<()> {
        let tool_name = self.name().to_string();
        let bin = self.exec_path().await?;
        let bin = CString::new(bin.to_str().unwrap()).unwrap();
        let exec_args = iter::once(&tool_name)
            .chain(args.into_iter())
            .map(|s| CString::new(s.as_bytes()).unwrap())
            .collect::<Vec<_>>();
        execve(&bin, &exec_args, &self.get_exec_env().await?)?;
        Ok(())
    }

    fn downloader(&self) -> &Downloader {
        self.toolbox.downloader()
    }
    pub async fn find_latest_version(&self) -> Result<String> {
        let url = self.definition.upstream.version_url();
        let response = self
            .downloader()
            .string(&url, &format!("{} check latest", self.name()))
            .await?;
        let latest_version = self
            .definition
            .upstream
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
        let url = self.definition.upstream.package_url(&version);

        let mut stream = self
            .downloader()
            .stream(&url, &format!("{}-{}", self.name(), version))
            .await?;

        fs::create_dir_all(&self.exec_dir_path().await?).await?;
        let mut file = File::create(&temp_bin_path).await?;

        let extract_command = self.definition.extract_command(&version);
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

    pub fn get_args<'b>(matches: &'b clap::ArgMatches) -> clap::Values<'b> {
        matches
            .values_of(ARGS_NAME)
            .or_else(|| matches.values_of(""))
            .unwrap_or_default()
    }

    pub fn subcommand(&self) -> clap::Command<'a> {
        let command = clap::Command::new(self.name())
            .bin_name(self.name())
            .allow_hyphen_values(true)
            .disable_help_flag(true)
            .disable_help_subcommand(true)
            .arg(Arg::with_name(ARGS_NAME).multiple(true))
            .about(&*self.definition.description);

        let command = if let Some(alias) = &self.definition.alias {
            command.alias(&**alias)
        } else {
            command
        };

        command
    }
}
