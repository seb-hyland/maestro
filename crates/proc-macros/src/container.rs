use oci_distribution::{Client, Reference, secrets::RegistryAuth};
use std::{path::Path, str::FromStr};
use tokio::runtime::Runtime;

pub(crate) fn check_manifest(oci: &str) -> Result<(), String> {
    let client = Client::default();
    let image =
        Reference::from_str(oci).map_err(|e| format!("Failed to parse image manifest!\n{e}"))?;
    let rt = Runtime::new().map_err(|e| format!("Failed to start async runtime!\n{e}"))?;

    rt.block_on(client.pull_manifest(&image, &RegistryAuth::Anonymous))
        .map(drop)
        .map_err(|e| format!("Image existence could not be verified!\n{e}"))
}

pub(crate) fn verify_sif(sif: &str) -> Result<(), String> {
    match Path::new(sif).exists() {
        false => Err(format!("File {sif} could not be found!")),
        true => Ok(()),
    }
}
