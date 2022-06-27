use crate::{result::Result, toolbox::Toolbox};
use clap::StructOpt;
use k8s_openapi::{
    api::core::v1::{Pod, SecurityContext},
    Metadata,
};
use kube_client::{
    api::{DeleteParams, ListParams, PostParams},
    core::WatchEvent,
    Api, Client, ResourceExt,
};
use log::info;

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

    #[clap(short, long, action)]
    privileged: bool,

    #[clap(default_value = "/bin/sh")]
    args: Vec<String>,
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

        let mut p: Pod = serde_yaml::from_value(serde_yaml::from_str(
            r#"
apiVersion: "v1"
kind: "Pod"
metadata:
    name: ""
spec:
    containers:
    - name: "shell"
      image: ""
      command:
      - /bin/sleep
      - infinity
"#,
        )?)?;
        p.metadata.name = Some(pod_name.clone());
        let spec = p.spec.as_mut().unwrap();
        spec.containers[0].image = Some(self.image.clone());
        spec.containers[0].security_context = Some(SecurityContext {
            privileged: Some(self.privileged),
            ..Default::default()
        });

        let pods: Api<Pod> = Api::namespaced(client, &namespace);
        // Stop on error including a pod already exists or is still being deleted.
        pods.create(&PostParams::default(), &p).await?;

        // Wait until the pod is running, otherwise we get 500 error.
        let lp = ListParams::default()
            .fields(&format!("metadata.name={}", p.name()))
            .timeout(10);
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
        let args: Vec<_> =
            vec!["exec", "-it", "--namespace", &namespace, &pod_name, "--"]
                .into_iter()
                .map(String::from)
                .chain(self.args.into_iter())
                .collect();

        kubectl.command(&args).await?.spawn()?.wait().await?;

        // Delete it
        println!("deleting");
        pods.delete(&p.name(), &DeleteParams::default())
            .await?
            .map_left(|pdel| {
                assert_eq!(pdel.name(), p.metadata.name.unwrap());
            });

        Ok(())
    }
}
