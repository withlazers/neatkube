use std::{
    env,
    ffi::{CString, OsStr, OsString},
    os::unix::prelude::{OsStrExt, PermissionsExt},
    path::PathBuf,
    process::Stdio,
    str::FromStr,
};

use crate::download;
use crate::error::Error;
use crate::result::Result;
use clap::Arg;
use nix::unistd::execve;
use serde::Deserialize;
use tokio::{
    fs, fs::File, io::AsyncReadExt, io::AsyncWriteExt, process::Command,
};
use tokio_stream::StreamExt;

use super::{
    upstream::{Upstream, UpstreamDefinition},
    Toolbox,
};
use dewey::VersionCmp;

#[derive(Debug, Clone)]
pub enum VersionRef {
    Latest,
    Local,
    Specific(String),
}

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
    pub extract_command: String,
    pub description: String,
    pub alias: Option<String>,
}

pub struct Tool<'a> {
    toolbox: &'a Toolbox,
    definition: &'a ToolDefinition,
}

impl<'a> Tool<'a> {
    pub fn new(definition: &'a ToolDefinition, toolbox: &'a Toolbox) -> Self {
        Self {
            definition,
            toolbox,
        }
    }
    pub fn name(&self) -> &str {
        &self.definition.name
    }

    pub fn description(&self) -> &str {
        &self.definition.description
    }

    async fn exec_dir_path(&self) -> Result<PathBuf> {
        let path = self.toolbox.exec_dir_path()?;
        fs::create_dir_all(&path).await?;

        Ok(path)
    }

    async fn exec_path<V>(&self, version_ref: &V) -> Result<PathBuf>
    where
        V: Into<VersionRef> + Clone,
    {
        let name = &self.name();
        let version = self.resolve_version_ref(version_ref).await?;
        Ok(self
            .exec_dir_path()
            .await?
            .join(format!("{}-{}", name, version)))
    }

    pub async fn find_local_version(&self) -> Result<Option<String>> {
        let all_versions = self.find_local_versions().await?;

        Ok(all_versions.into_iter().last())
    }

    pub async fn resolve_version_ref<V>(
        &self,
        version_ref: &V,
    ) -> Result<String>
    where
        V: Into<VersionRef> + Clone,
    {
        match version_ref.clone().into() {
            VersionRef::Latest => self.find_latest_version().await,
            VersionRef::Local => match self.find_local_version().await? {
                Some(version) => Ok(version),
                None => self.find_latest_version().await,
            },
            VersionRef::Specific(version) => Ok(version),
        }
    }

    pub async fn run(&self, args: &[&str]) -> Result<()> {
        self.install(&VersionRef::Local, false).await?;

        self.exec(args).await
    }

    fn get_exec_env(&self) -> Result<Vec<CString>> {
        let bin_dir_path = self.toolbox.bin_dir_path()?;
        let bin_dir_path: &OsStr = bin_dir_path.as_ref();
        let new_env = env::vars_os()
            .into_iter()
            .map(|(name, value)| match name.to_str() {
                Some("PATH") => {
                    let mut value = value.to_owned();
                    value.push(":");
                    value.push(bin_dir_path);
                    (name, value)
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

    pub async fn exec(&self, args: &[&str]) -> Result<()> {
        let mut exec_args = vec![self.name()];
        exec_args.extend(args.iter());
        let bin = self.exec_path(&VersionRef::Local).await?;
        let bin = CString::new(bin.to_str().unwrap()).unwrap();
        let exec_args = exec_args
            .into_iter()
            .map(|s| CString::new(s).unwrap())
            .collect::<Vec<_>>();
        execve(&bin, &exec_args, &self.get_exec_env()?)?;
        Ok(())
    }

    pub async fn find_latest_version(&self) -> Result<String> {
        let url = self.definition.upstream.version_url();
        let response = download::string(&url).await?;
        self.definition
            .upstream
            .parse_version_from_response(&response)
    }

    pub async fn find_local_versions(&self) -> Result<Vec<String>> {
        let bin_dir = self.exec_dir_path().await?;
        let mut versions = vec![];
        let prefix = format!("{}-", self.name());
        let mut dir = fs::read_dir(&bin_dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let version_name = entry
                .file_name()
                .to_str()
                .and_then(|x| x.strip_prefix(&prefix))
                .filter(|x| !x.ends_with(".part"))
                .map(|x| x.to_string());

            if let Some(file_name) = version_name {
                versions.push(file_name.to_string());
            }
        }
        versions.sort_by(|a, b| a.ver_cmp(b).unwrap());
        Ok(versions)
    }

    pub async fn install<V>(&self, version_ref: &V, force: bool) -> Result<bool>
    where
        V: Into<VersionRef> + Clone,
    {
        if self.is_installed(version_ref).await? && !force {
            return Ok(false);
        }
        let version = self.resolve_version_ref(version_ref).await?;
        let bin_path = self.exec_path(&version.as_str()).await?;
        let temp_bin_path = bin_path.with_extension("part");
        let url = self.definition.upstream.package_url(&version);

        let mut stream = download::stream(&url).await?;
        let mut file = File::create(&temp_bin_path).await?;

        if self.definition.extract_command.is_empty() {
            while let Some(chunk) = stream.next().await {
                file.write_all(&chunk?).await?;
            }
        } else {
            let cmd = &self.definition.extract_command;
            let mut process = Command::new("sh")
                .arg("-c")
                .arg(cmd)
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

    async fn is_installed<V>(&self, version_ref: &V) -> Result<bool>
    where
        V: Into<VersionRef> + Clone,
    {
        // TODO: Blocking function
        Ok(self.exec_path(version_ref).await?.exists())
    }

    pub fn subcommand(&self) -> clap::Command<'a> {
        let command = clap::Command::new(self.name())
            .allow_hyphen_values(true)
            .disable_help_flag(true)
            .disable_help_subcommand(true)
            .arg(Arg::with_name("").multiple(true))
            .about(&*self.definition.description);

        let command = if let Some(alias) = &self.definition.alias {
            command.alias(&**alias)
        } else {
            command
        };

        command
    }
}
