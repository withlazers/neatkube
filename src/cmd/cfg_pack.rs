use std::io::Read;

use base64::Engine;
use clap::Parser;
use kube_client::config::Kubeconfig;
use secrecy::Secret;

#[derive(Debug, Parser)]
#[clap(
    name = "cfgpack",
    about = "Inlines references in kubeconfig into the config"
)]
pub struct CfgPackCommand {
    /// KUBECONFIG
    #[clap(name = "KUBECONFIG", env = "KUBECONFIG")]
    kube_config: Option<String>,
}

impl CfgPackCommand {
    fn inline_secret(
        path: &mut Option<String>,
        data: &mut Option<Secret<String>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut secret = None;
        Self::inline(path, &mut secret)?;
        if let Some(string) = secret {
            *data = Some(Secret::new(string));
        }
        Ok(())
    }

    fn inline(
        path: &mut Option<String>,
        data: &mut Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(path_to_inline) = path {
            let mut file = std::fs::File::open(path_to_inline)?;
            let mut contents = vec![];
            let engine = &base64::engine::general_purpose::STANDARD;
            file.read_to_end(&mut contents)?;
            *path = None;
            *data = Some(engine.encode(contents));
        }
        Ok(())
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let mut config = if let Some(kube_config) = self.kube_config {
            Kubeconfig::read_from(kube_config)?
        } else {
            Kubeconfig::read()?
        };

        for cluster in config.clusters.iter_mut() {
            let Some(cluster) = &mut cluster.cluster else {
                continue;
            };
            Self::inline(
                &mut cluster.certificate_authority,
                &mut cluster.certificate_authority_data,
            )?;
        }
        for auth_info in config.auth_infos.iter_mut() {
            let Some(auth_info) = &mut auth_info.auth_info else {
                continue;
            };
            Self::inline_secret(
                &mut auth_info.client_key,
                &mut auth_info.client_key_data,
            )?;
            Self::inline(
                &mut auth_info.client_certificate,
                &mut auth_info.client_certificate_data,
            )?;
        }

        serde_yaml::to_writer(std::io::stdout(), &config)?;
        Ok(())
    }
}
