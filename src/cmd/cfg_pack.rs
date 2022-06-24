use std::io::Read;

use clap::StructOpt;
use kube_client::config::Kubeconfig;
use secrecy::Secret;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "cfgpack",
    about = "Inlines references in kubeconfig into the config"
)]
pub struct CfgPackCommand {
    /// KUBECONFIG
    #[structopt(name = "KUBECONFIG", env = "KUBECONFIG")]
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
            file.read_to_end(&mut contents)?;
            *path = None;
            *data = Some(base64::encode(contents));
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
            let cluster = &mut cluster.cluster;
            Self::inline(
                &mut cluster.certificate_authority,
                &mut cluster.certificate_authority_data,
            )?;
        }
        for auth_info in config.auth_infos.iter_mut() {
            let auth_info = &mut auth_info.auth_info;
            Self::inline_secret(
                &mut auth_info.client_key,
                &mut auth_info.client_key_data,
            )?;
            Self::inline(
                &mut auth_info.client_certificate,
                &mut auth_info.client_certificate_data,
            )?;
        }

        println!("{:?}", config);
        serde_yaml::to_writer(std::io::stdout(), &config)?;
        Ok(())
    }
}
