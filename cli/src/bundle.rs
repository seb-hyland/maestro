use crate::{
    StringResult,
    build::{BuildType, build_project},
    find_crate_root, mapper, report_process_failure,
};
use clap::ValueEnum;
use std::{
    env,
    ffi::OsStr,
    fs, io,
    os::unix::ffi::OsStrExt,
    path::Path,
    process::{Command, Stdio},
};

#[derive(Clone, Copy, ValueEnum)]
/// Compression options
pub enum Compression {
    Zip,
    Gzip,
    Xz,
    Bzip2,
    Zstd,
    Lzma,
}

#[derive(Clone, Copy, ValueEnum)]
/// Architecture build targets
pub enum Arch {
    Linux,
    Apple,
    All,
}

#[derive(Clone, Copy, ValueEnum)]
/// Container runtimes to use for multiarch builds
pub enum ContainerRuntime {
    Docker,
    Podman,
    Apptainer,
}

pub(crate) fn bundle_project(
    cargo_args: Vec<String>,
    compression: Option<Compression>,
    arch: Option<Arch>,
    runtime: ContainerRuntime,
) -> StringResult {
    let crate_root = find_crate_root()?;
    let bundle_dir = crate_root.join("maestro_bundle");

    if bundle_dir.exists() {
        fs::remove_dir_all(&bundle_dir)
            .map_err(|e| mapper(&e, "Failed to clean maestro_bundle directory"))?;
    }
    fs::create_dir_all(&bundle_dir)
        .map_err(|e| mapper(&e, "Failed to create maestro_bundle directory"))?;

    match arch {
        None => {
            build_project(cargo_args, Vec::new(), BuildType::Build)?;

            let target_dir = crate_root.join("target/release");
            for file in fs::read_dir(&target_dir)
                .map_err(|e| mapper(&e, "Failed to read contents of build directory"))?
                .flatten()
            {
                let path = file.path();
                if path.extension() == Some(OsStr::from_bytes(b"d")) {
                    let binary_path = path.with_extension("");
                    fs::copy(
                        &binary_path,
                        bundle_dir.join(binary_path.file_name().unwrap()),
                    )
                    .map_err(|e| mapper(&e, "Failed to copy build outputs to bundle directory"))?;
                }
            }
        }
        Some(arch) => {
            const APPLE: [&str; 2] = ["x86_64-apple-darwin", "aarch64-apple-darwin"];
            const LINUX: [&str; 2] = ["x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl"];

            let runtime_cmd = match runtime {
                ContainerRuntime::Docker => "docker",
                ContainerRuntime::Podman => "podman",
                ContainerRuntime::Apptainer => "apptainer",
            };
            let args: &[&str] = match runtime {
                ContainerRuntime::Docker | ContainerRuntime::Podman => &[
                    "run",
                    "--rm",
                    "-v",
                    &format!("{}:/io:z", crate_root.display()),
                    "-w",
                    "/io",
                ],
                ContainerRuntime::Apptainer => &[
                    "exec",
                    "--writable",
                    "--bind",
                    &format!("{}:/io", crate_root.display()),
                    "--workdir",
                    "/io",
                ],
            };
            let additional_flags: &[&str] = match runtime {
                ContainerRuntime::Docker => &[
                    "--user",
                    &format!(
                        "{}:{}",
                        env::var("UID").unwrap_or("1000".to_owned()),
                        env::var("GID").unwrap_or("1000".to_owned())
                    ),
                ],
                _ => &[],
            };
            let image_cmd = match runtime {
                ContainerRuntime::Docker | ContainerRuntime::Podman => "maestro_build",
                ContainerRuntime::Apptainer => "docker://maestro_build",
            };
            let arches = match arch {
                Arch::Linux => &LINUX,
                Arch::Apple => &APPLE,
                Arch::All => &LINUX.into_iter().chain(APPLE).collect::<Vec<&str>>()[..],
            };
            let copy_cmds = {
                let mut copy_str = String::new();
                for arch in arches {
                    copy_str.push_str(&format!(
                        "for f in target/{arch}/release/*; do \
                        if [[ -f $f && -x $f ]]; then \
                        fname=$(basename $f); \
                        cp $f /io/maestro_bundle/${{fname}}_{arch}; \
                        fi; \
                        done; ",
                    ));
                }
                copy_str
            };
            let bash_args = [
                "bash",
                "-c",
                &format!(
                    "rsync -a --exclude={{'.*','target'}} /io/ /tmp/build/ && \
                    cd /tmp/build/ && \
                    cargo zigbuild --release {} {} && \
                    {copy_cmds} \
                    cp procinfo.toml /io/procinfo.toml",
                    arches.iter().fold(String::new(), |mut acc, arch| {
                        acc.push_str(" --target ");
                        acc.push_str(arch);
                        acc
                    }),
                    cargo_args.join(" ")
                ),
            ];

            let cmd = Command::new(runtime_cmd)
                .args(args)
                .args(additional_flags)
                .arg(image_cmd)
                .args(bash_args)
                .status()
                .map_err(|e| mapper(&e, "Failed to spawn container for multi-arch build"))?;
            if !cmd.success() {
                return Err(report_process_failure(cmd, "Multi-arch build container"));
            }
        }
    }

    fs::copy(
        crate_root.join("Maestro.toml"),
        bundle_dir.join("Maestro.toml"),
    )
    .map_err(|e| mapper(&e, "Failed to copy Maestro.toml to bundle directory"))?;
    fs::copy(
        crate_root.join("procinfo.toml"),
        bundle_dir.join("procinfo.toml"),
    )
    .map_err(|e| mapper(&e, "Failed to copy procinfo.toml to bundle directory"))?;

    fn copy_recursively(src: &Path, dst: &Path) -> io::Result<()> {
        fs::create_dir(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            let entry_name = entry.file_name();
            if ty.is_dir() {
                copy_recursively(&entry.path(), &dst.join(entry_name))?;
            } else {
                fs::copy(entry.path(), dst.join(entry_name))?;
            }
        }
        Ok(())
    }
    copy_recursively(&crate_root.join("data/"), &bundle_dir.join("data/"))
        .map_err(|e| mapper(&e, "Failed to copy data/ to bundle directory"))?;

    if let Some(compression) = compression {
        let bundle_name = bundle_dir.file_name().unwrap();
        let mut command = match compression {
            Compression::Zip => {
                let mut cmd = Command::new("zip");
                cmd.current_dir(&crate_root)
                    .args(["-r", "maestro_bundle.zip"])
                    .arg(bundle_name);
                cmd
            }
            Compression::Gzip => {
                let mut cmd = Command::new("tar");
                cmd.current_dir(&crate_root)
                    .args(["-czf", "maestro_bundle.tar.gz"])
                    .arg(bundle_name);
                cmd
            }
            Compression::Xz => {
                let mut cmd = Command::new("tar");
                cmd.current_dir(&crate_root)
                    .args(["-cJf", "maestro_bundle.tar.xz"])
                    .arg(bundle_name);
                cmd
            }
            Compression::Bzip2 => {
                let mut cmd = Command::new("tar");
                cmd.current_dir(&crate_root)
                    .args(["-cjf", "maestro_bundle.tar.bz2"])
                    .arg(bundle_name);
                cmd
            }
            Compression::Zstd => {
                let mut cmd = Command::new("tar");
                cmd.current_dir(&crate_root)
                    .args(["--zstd", "-cf", "maestro_bundle.tar.zst"])
                    .arg(bundle_name);
                cmd
            }
            Compression::Lzma => {
                let mut cmd = Command::new("tar");
                cmd.current_dir(&crate_root)
                    .args(["--lzma", "-cf", "maestro_bundle.tar.lzma"])
                    .arg(bundle_name);
                cmd
            }
        };
        let status = command
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| mapper(&e, "Failed to run compression"))?;
        if !status.success() {
            return Err(report_process_failure(status, "Compression"));
        }
    }

    Ok(())
}
