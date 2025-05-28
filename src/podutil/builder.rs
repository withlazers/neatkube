use k8s_openapi::api::core::v1::{
    ConfigMapVolumeSource, Container, HostPathVolumeSource,
    PersistentVolumeClaimVolumeSource, Pod, PodSpec, SecretVolumeSource,
    SecurityContext, Volume, VolumeMount,
};
use kube_client::api::ObjectMeta;

pub struct PodBuilder {
    metadata: ObjectMeta,
    spec: PodSpec,
}

impl Default for PodBuilder {
    fn default() -> Self {
        PodBuilder {
            metadata: ObjectMeta {
                ..Default::default()
            },
            spec: PodSpec {
                containers: vec![Container {
                    volume_mounts: Some(vec![]),
                    command: Some(vec![
                        "/bin/sleep".to_string(),
                        "infinity".to_string(),
                    ]),
                    ..Default::default()
                }],
                volumes: Some(vec![]),
                ..Default::default()
            },
        }
    }
}

impl PodBuilder {
    pub fn new() -> PodBuilder {
        Self::default()
    }

    fn container_mut(&mut self) -> &mut Container {
        &mut self.spec.containers[0]
    }

    pub fn name(&mut self, name: &str) -> &mut PodBuilder {
        self.metadata.name = Some(name.to_string());
        self.container_mut().name = name.to_string();
        self
    }

    pub fn image(&mut self, image: &str) -> &mut Self {
        self.container_mut().image = Some(image.to_string());
        self
    }

    pub fn host_ipc(&mut self, host_ipc: bool) -> &mut Self {
        self.spec.host_ipc = host_ipc.then_some(true);
        self
    }

    pub fn service_account(&mut self, service_account: &str) -> &mut Self {
        self.spec.service_account_name = Some(service_account.to_string());
        self
    }

    pub fn host_network(&mut self, host_network: bool) -> &mut Self {
        self.spec.host_network = host_network.then_some(true);
        self
    }

    pub fn privileged(&mut self, privileged: bool) -> &mut Self {
        self.container_mut().security_context = if privileged {
            Some(SecurityContext {
                privileged: Some(true),
                ..Default::default()
            })
        } else {
            None
        };
        self
    }

    pub fn host_pid(&mut self, host_pid: bool) -> &mut Self {
        self.spec.host_pid = host_pid.then_some(true);
        self
    }

    pub fn labels<H>(&mut self, labels: H) -> &mut Self
    where
        H: IntoIterator<Item = (String, String)>,
    {
        self.metadata.labels = Some(labels.into_iter().collect());
        self
    }

    pub fn annotations<H>(&mut self, annotations: H) -> &mut Self
    where
        H: IntoIterator<Item = (String, String)>,
    {
        self.metadata.annotations = Some(annotations.into_iter().collect());
        self
    }

    pub fn config_map(
        &mut self,
        name: &str,
        mount_path: Option<&String>,
    ) -> &mut Self {
        self.volume(
            Volume {
                config_map: Some(ConfigMapVolumeSource {
                    name: Some(name.to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            mount_path.unwrap_or(&format!("/configmap/{}", name)),
        );
        self
    }

    pub fn secret(
        &mut self,
        name: &str,
        mount_path: Option<&String>,
    ) -> &mut Self {
        self.volume(
            Volume {
                secret: Some(SecretVolumeSource {
                    secret_name: Some(name.to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            mount_path.unwrap_or(&format!("/secret/{}", name)),
        );
        self
    }

    pub fn host_dir(
        &mut self,
        path: &str,
        mount_path: Option<&String>,
    ) -> &mut Self {
        self.volume(
            Volume {
                host_path: Some(HostPathVolumeSource {
                    path: path.to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            mount_path.unwrap_or(&format!("/host/{}", path)),
        );
        self
    }

    pub fn pvc(
        &mut self,
        name: &str,
        mount_path: Option<&String>,
    ) -> &mut Self {
        self.volume(
            Volume {
                persistent_volume_claim: Some(
                    PersistentVolumeClaimVolumeSource {
                        claim_name: name.to_string(),
                        ..Default::default()
                    },
                ),
                ..Default::default()
            },
            mount_path.unwrap_or(&format!("/pvc/{}", name)),
        );
        self
    }

    fn volume(&mut self, mut volume: Volume, mount_path: &str) -> &mut Self {
        let volumes = self.spec.volumes.as_mut().unwrap();
        let index = volumes.len();
        let new_name = format!("volume-{}", index);
        volume.name = new_name.clone();
        volumes.push(volume);

        let volume_mounts =
            self.container_mut().volume_mounts.as_mut().unwrap();
        volume_mounts.push(VolumeMount {
            mount_path: mount_path.to_string(),
            name: new_name,
            mount_propagation: Some("Bidirectional".to_string()),
            ..Default::default()
        });
        self
    }

    pub fn node_selector(&mut self, node: &str) -> &mut Self {
        self.spec.node_selector = Some(
            [("kubernetes.io/hostname".to_string(), node.to_string())].into(),
        );
        self
    }

    pub fn build(self) -> Pod {
        Pod {
            metadata: self.metadata,
            spec: self.spec.into(),
            ..Default::default()
        }
    }
}
