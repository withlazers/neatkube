use serde_yaml::Value;

use std::{fmt::{Debug, Display, Formatter}, fs::File, io::Read, path::PathBuf, str::FromStr};
use clap::Parser;

#[derive(Debug)]
struct KubeCfgPath(PathBuf);

impl FromStr for KubeCfgPath {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(KubeCfgPath(PathBuf::from(s)))
    }
}

impl Display for KubeCfgPath {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Default for KubeCfgPath {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap();
        KubeCfgPath(format!("{}/.kube/config", home).into())
    }
}

#[derive(Debug, Parser)]
#[clap(about = "Inlines references in kubeconfig into the config")]
pub struct Opt {
    /// KUBECONFIG
    #[clap(name = "KUBECONFIG", env = "KUBECONFIG", default_value)]
    kube_config: KubeCfgPath,
}

fn replace_refs(config: &mut Value, key: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(obj) = config.as_mapping_mut() {
        let key_value = Value::String(key.to_string());
        if let Some(Value::String(s)) = obj.get(&key_value) {
            if let Ok(ref mut file) = File::open(s) {
                let mut contents = vec![];
                file.read_to_end(&mut contents)?;
                obj.remove(&key_value);
                let new_key = Value::String(format!("{}-data", key));
                obj.insert(new_key, Value::String(base64::encode(contents)));
            }
        }
    }
    Ok(())
}

fn replace_cluster(clusters: &mut Value) -> Result<(), Box<dyn std::error::Error>> {
    for cluster in clusters.as_sequence_mut().unwrap_or(&mut Vec::new()) {
        if let Some(cluster) = cluster.get_mut("cluster") {
            replace_refs(cluster, "certificate-authority")?;
        }
    }
    Ok(())
}

fn replace_users(users: &mut Value) -> Result<(), Box<dyn std::error::Error>> {
    for user in users.as_sequence_mut().unwrap_or(&mut Vec::new()) {
        if let Some(user) = user.get_mut("user") {
            replace_refs(user, "client-certificate")?;
            replace_refs(user, "client-key")?;
        }
    }
    Ok(())
}

pub fn run(opt: Opt) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(opt.kube_config.0)?;
    let mut config: Value = serde_yaml::from_reader(file)?;
    replace_cluster(&mut config["clusters"])?;
    replace_users(&mut config["users"])?;
    serde_yaml::to_writer(std::io::stdout(), &config)?;
    Ok(())
}

