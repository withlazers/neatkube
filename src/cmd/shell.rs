use std::convert::Infallible;

use crate::{
    error::TError,
    podutil::{builder::PodBuilder, exec::PodExec, upload::PodUpload},
    result::Result,
    toolbox::Toolbox,
};
use clap::Parser;
use k8s_openapi::api::core::v1::Pod;
use kube_client::{
    api::{DeleteParams, ListParams, PostParams},
    core::WatchEvent,
    Api, Client, Config, ResourceExt,
};
use log::info;
use std::result::Result as StdResult;
use tokio::task;

use randstr::randstr;

use futures::{StreamExt, TryStreamExt};

#[derive(Parser, Debug)]
#[clap(name = "shell", about = "drops you to a temporary shell on a cluster")]
pub struct ShellCommand {
    /// container image to start
    #[clap(short, long, default_value = concat!("withlazers/", env!("CARGO_PKG_NAME"), ":v", env!("CARGO_PKG_VERSION")))]
    image: String,

    /// edit yaml before starting the pod
    #[clap(short = 'e', long, action)]
    edit: bool,

    /// namespace to use, default is infered
    #[clap(short, long, env = "NAMESPACE")]
    namespace: Option<String>,

    /// node to run on
    #[clap(short = 'm', long)]
    node: Option<String>,

    /// share network namespace with host
    #[clap(short = 'N', long, action)]
    host_network: bool,

    /// share ipc namespace with host
    #[clap(short = 'I', long, action)]
    host_ipc: bool,

    /// share pid namespace with host
    #[clap(short = 'P', long, action)]
    host_pid: bool,

    /// start container in privileged mode
    #[clap(short, long, action)]
    privileged: bool,

    /// service account to use
    #[clap(short = 'A', long, name = "ACCOUNT")]
    service_account: Option<String>,

    /// mounts a PVC. if no path is given, the secret will be mounted at /pvc
    #[clap(short = 'v', long = "pvc", value_parser = volume_parser, name = "PVC[:PATH]")]
    pvcs: Vec<(String, Option<String>)>,

    /// mounts a secret. if no path is given, the secret will be mounted at /secret
    #[clap(short, long = "secret", value_parser = volume_parser, name = "SECRET[:PATH]")]
    secrets: Vec<(String, Option<String>)>,

    /// mounts the host
    #[clap(short = 'H', long, value_parser = volume_parser, name = "HPATH[:PATH]")]
    hostdir: Vec<(String, Option<String>)>,

    #[clap(short, long, value_parser = volume_parser, name = "CMAP[:PATH]")]
    config_map: Vec<(String, Option<String>)>,

    #[clap(short, long, value_parser = upload_parser, name = "LOCAL:PATH")]
    upload: Vec<(String, String)>,

    #[clap(short = 'l', long, value_parser = kv_parser, name = "LABEL=VALUE")]
    label: Vec<(String, String)>,

    #[clap(short = 'a', long, value_parser = kv_parser, name = "ANNOTATION=VALUE")]
    annotation: Vec<(String, String)>,

    #[clap(default_value = "/bin/sh")]
    args: Vec<String>,

    /// do not terminate the pod on exit
    #[clap(short, long, action)]
    keep: bool,
}

fn kv_parser(input: &str) -> StdResult<(String, String), TError> {
    let volume = match &input.split_once('=') {
        Some((name, path)) => (name.to_string(), path.to_string()),
        None => Err("Must be key=value")?,
    };
    Ok(volume)
}

fn upload_parser(input: &str) -> StdResult<(String, String), TError> {
    let (local, path) = volume_parser(input)?;
    Ok((
        local,
        path.ok_or_else(|| {
            "upload directories must be in the form of LOCAL:PATH".to_string()
        })?,
    ))
}

fn volume_parser(
    input: &str,
) -> StdResult<(String, Option<String>), Infallible> {
    let volume = match &input.split_once(':') {
        Some((name, path)) => (name.to_string(), path.to_string().into()),
        None => (input.to_string(), None),
    };
    Ok(volume)
}

impl ShellCommand {
    pub async fn run(self, toolbox: &Toolbox) -> Result<()> {
        let config = Config::infer().await?;
        let client = Client::try_from(config.clone())?;
        let mut random = randstr().len(6).lower().digit().try_build()?;
        let namespace = self
            .namespace
            .as_ref()
            .unwrap_or(&config.default_namespace)
            .clone();
        let pod_name =
            format!("{}-shell-{}", env!("CARGO_PKG_NAME"), random.generate());

        let mut pod_builder = PodBuilder::new();
        pod_builder
            .name(&pod_name)
            .image(&self.image)
            .host_ipc(self.host_ipc)
            .host_network(self.host_network)
            .host_pid(self.host_pid)
            .labels(self.label.clone())
            .annotations(self.annotation.clone());

        for (secret, path) in &self.secrets {
            pod_builder.secret(secret, path.as_ref());
        }
        for (config_map, path) in &self.config_map {
            pod_builder.config_map(config_map, path.as_ref());
        }
        for (pvc, path) in &self.pvcs {
            pod_builder.pvc(pvc, path.as_ref());
        }
        for (host_dir, path) in &self.hostdir {
            pod_builder.host_dir(host_dir, path.as_ref());
        }

        if let Some(node_selector) = self.node.as_ref() {
            pod_builder.node_selector(node_selector);
        }

        pod_builder.annotations(self.annotation.clone());
        pod_builder.labels(self.label.clone());

        let pod = pod_builder.build();
        let pod = if self.edit {
            self.edit_pod(pod).await?
        } else {
            pod
        };
        let container_name =
            &pod.spec.as_ref().unwrap().containers.first().unwrap().name;

        let pods: Api<Pod> = Api::namespaced(client, &namespace);
        pods.create(&PostParams::default(), &pod).await?;

        tokio::select! {
            r = self.wait_for_pod(&pods, &pod_name) => r?,
            _ = tokio::signal::ctrl_c() => {
                self.delete_pod(&pods, &pod_name).await?;
                std::process::exit(130);
            }
        }

        for (local, remote) in self.upload.iter() {
            PodUpload::new(toolbox)
                .name(&pod.name())
                .namespace(&namespace)
                .container_name(container_name)
                .upload(local, remote)
                .await?;
        }

        PodExec::new(toolbox)
            .terminal(true)
            .name(&pod.name())
            .namespace(&namespace)
            .container_name(container_name)
            .command(self.args.clone())
            .await?
            .spawn()?
            .wait()
            .await?;

        if !self.keep {
            self.delete_pod(&pods, &pod_name).await?;
        }

        Ok(())
    }

    async fn delete_pod(&self, pods: &Api<Pod>, name: &str) -> Result<()> {
        pods.delete(name, &DeleteParams::default())
            .await?
            .map_left(|pdel| {
                assert_eq!(pdel.name(), name);
            });
        Ok(())
    }

    async fn wait_for_pod(&self, pods: &Api<Pod>, name: &str) -> Result<()> {
        let lp =
            ListParams::default().fields(&format!("metadata.name={}", name));
        let mut stream = pods.watch(&lp, "0").await?.boxed();
        while let Some(status) = stream.try_next().await? {
            match status {
                WatchEvent::Added(o) => {
                    info!("Added {}", o.name());
                }
                WatchEvent::Modified(o) => {
                    let s = o.status.as_ref().expect("status exists on pod");
                    if s.phase.clone().unwrap_or_default() == "Running" {
                        info!("Ready to attach to {}", o.name());
                        break;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn edit_pod(&self, pod: Pod) -> Result<Pod> {
        Ok(task::spawn_blocking(move || {
            let mut pod_content = serde_yaml::to_string(&pod).unwrap();
            let mut builder = edit::Builder::new();
            builder.prefix("pod-").suffix(".yaml");
            let mut err_msg = String::new();
            loop {
                let content = if err_msg.is_empty() {
                    pod_content.clone()
                } else {
                    format!(
                        "{}\n{}",
                        err_msg
                            .split_inclusive('\n')
                            .map(|x| format!("# {}", x))
                            .collect::<String>(),
                        pod_content
                    )
                };
                let content =
                    edit::edit_with_builder(content, &builder).unwrap();
                match serde_yaml::from_str::<Pod>(&content) {
                    Ok(x) => return x,
                    Err(x) => {
                        err_msg = x.to_string();
                        pod_content = content
                            .split_inclusive('\n')
                            .skip_while(|x| x.starts_with("# "))
                            .collect();
                    }
                }
            }
        })
        .await?)
    }
}
