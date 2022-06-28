use std::{convert::Infallible, iter};

use crate::{result::Result, toolbox::Toolbox};
use clap::StructOpt;
use k8s_openapi::api::core::v1::{
    ConfigMapVolumeSource, Container, HostPathVolumeSource, Pod, PodSpec,
    SecretVolumeSource, SecurityContext, Volume, VolumeMount,
};
use kube_client::{
    api::{DeleteParams, ListParams, PostParams},
    core::{ObjectMeta, WatchEvent},
    Api, Client, ResourceExt,
};
use log::info;
use std::result::Result as StdResult;

use futures::{StreamExt, TryStreamExt};

#[derive(StructOpt, Debug)]
#[structopt(
    name = "shell",
    about = "drops you to a temporary shell on a cluster"
)]
pub struct ShellCommand {
    #[clap(short, long, default_value = "alpine")]
    image: String,

    #[clap(short, long, default_value = "default", env = "NAMESPACE")]
    namespace: String,

    #[clap(short, long, action, help = "make container privileged")]
    privileged: bool,

    #[clap(short, long = "secret", action, value_parser = secret_ref_parser, name = "SECRET[:PATH]")]
    secrets: Vec<(String, String, String)>,

    #[clap(short = 'H', long, action, value_parser = host_dir_parser, name = "HPATH[:PATH]")]
    hostdir: Option<(String, String, String)>,

    #[clap(short, long, action, value_parser = configmap_ref_parser, name = "CMAP[:PATH]")]
    config_maps: Vec<(String, String, String)>,

    #[clap(default_value = "/bin/sh")]
    args: Vec<String>,
}

fn host_dir_parser(
    input: &str,
) -> StdResult<(String, String, String), Infallible> {
    let host = "host".to_string();
    match &input.split_once(':') {
        Some((host_path, path)) => {
            Ok((host, host_path.to_string(), path.to_string()))
        }
        None => Ok((host, input.to_string(), "/host".to_string())),
    }
}

fn configmap_ref_parser(
    input: &str,
) -> StdResult<(String, String, String), Infallible> {
    ref_parser("configmap", input)
}

fn secret_ref_parser(
    input: &str,
) -> StdResult<(String, String, String), Infallible> {
    ref_parser("secret", input)
}

fn ref_parser(
    kind: &'static str,
    input: &str,
) -> StdResult<(String, String, String), Infallible> {
    let (name, path) = match &input.split_once(':') {
        Some((name, path)) => (*name, path.to_string()),
        None => (input, format!("/{}/{}", kind, input)),
    };

    let volume_name = format!("{}-{}", kind, name);
    Ok((volume_name, name.to_string(), path))
}

impl ShellCommand {
    pub async fn run(self, toolbox: &Toolbox) -> Result<()> {
        let client = Client::try_default().await?;
        let namespace = self.namespace;
        let pod_name = format!(
            "{}-shell-{}",
            env!("CARGO_PKG_NAME"),
            random_string::generate(6, "abcdefghijklmnopqrstuvwxyz1234567890")
        );

        let command = ["/bin/sleep", "infinity"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();

        let secret_volumes =
            self.secrets.iter().map(|(volume_name, name, _)| Volume {
                name: volume_name.clone(),
                secret: SecretVolumeSource {
                    secret_name: name.clone().into(),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            });

        let config_map_volumes =
            self.config_maps
                .iter()
                .map(|(volume_name, name, _)| Volume {
                    name: volume_name.clone(),
                    config_map: ConfigMapVolumeSource {
                        name: name.clone().into(),
                        ..Default::default()
                    }
                    .into(),
                    ..Default::default()
                });

        let host_dir_volumes =
            self.hostdir
                .iter()
                .map(|(volumen_name, host_path, _)| Volume {
                    name: volumen_name.clone(),
                    host_path: HostPathVolumeSource {
                        path: host_path.clone(),
                        ..Default::default()
                    }
                    .into(),
                    ..Default::default()
                });

        let volumes = iter::empty()
            .chain(secret_volumes)
            .chain(config_map_volumes)
            .chain(host_dir_volumes)
            .collect::<Vec<_>>();

        let volume_mounts = iter::empty()
            .chain(self.config_maps.iter())
            .chain(self.secrets.iter())
            .chain(self.hostdir.iter())
            .map(|(volume_name, _, path)| VolumeMount {
                name: volume_name.clone(),
                mount_path: path.clone(),
                ..Default::default()
            })
            .collect::<Vec<_>>();

        let pod = Pod {
            metadata: ObjectMeta {
                name: pod_name.clone().into(),
                ..Default::default()
            },
            spec: PodSpec {
                containers: vec![Container {
                    name: env!("CARGO_PKG_NAME").to_string(),
                    image: self.image.clone().into(),
                    command: command.into(),
                    security_context: SecurityContext {
                        privileged: Some(self.privileged),
                        ..Default::default()
                    }
                    .into(),
                    volume_mounts: volume_mounts.into(),
                    ..Default::default()
                }],
                volumes: volumes.into(),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        };

        let pods: Api<Pod> = Api::namespaced(client, &namespace);
        // Stop on error including a pod already exists or is still being deleted.
        pods.create(&PostParams::default(), &pod).await?;

        // Wait until the pod is running, otherwise we get 500 error.
        let lp = ListParams::default()
            .fields(&format!("metadata.name={}", pod.name()));
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

        let kubectl = toolbox.tool("kubectl")?;
        let args: Vec<_> = [
            "exec",
            "-it",
            "-c",
            env!("CARGO_PKG_NAME"),
            "--namespace",
            &namespace,
            &pod_name,
            "--",
        ]
        .into_iter()
        .map(String::from)
        .chain(self.args.into_iter())
        .collect();

        kubectl.command(&args).await?.spawn()?.wait().await?;

        // Delete it
        pods.delete(&pod.name(), &DeleteParams::default())
            .await?
            .map_left(|pdel| {
                assert_eq!(pdel.name(), pod.metadata.name.unwrap());
            });

        Ok(())
    }
}
