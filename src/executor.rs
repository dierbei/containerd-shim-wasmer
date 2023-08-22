use containerd_shim_wasm::sandbox::oci::{self, Spec};
use libc::{STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO};
use libcontainer::workload::{Executor, ExecutorError};
use nix::unistd::{dup, dup2};
use std::{os::fd::RawFd, path::PathBuf};
use wasmer::{Cranelift, Instance, Module, Store};
use wasmer_wasix::WasiEnv;

use crate::oci_wasmer;

const EXECUTOR_NAME: &str = "wasmer";

pub struct WasmerExecutor {
    stdin: Option<RawFd>,
    stdout: Option<RawFd>,
    stderr: Option<RawFd>,
    engine: Cranelift,
}

impl WasmerExecutor {
    pub fn new(
        stdin: Option<RawFd>,
        stdout: Option<RawFd>,
        stderr: Option<RawFd>,
        engine: Cranelift,
    ) -> Self {
        Self {
            stdin,
            stdout,
            stderr,
            engine,
        }
    }
}

impl Executor for WasmerExecutor {
    fn name(&self) -> &'static str {
        EXECUTOR_NAME
    }

    fn exec(&self, spec: &containerd_shim_wasm::sandbox::oci::Spec) -> Result<(), ExecutorError> {
        log::info!("wasmer executor exec method");

        let args = oci::get_args(spec);
        if args.is_empty() {
            return Err(ExecutorError::InvalidArg);
        }

        let _ = self
            .prepare(spec, args)
            .map_err(|err| ExecutorError::Other(format!("failed to prepare function: {}", err)))?;

        std::process::exit(0)
    }

    fn can_handle(&self, spec: &containerd_shim_wasm::sandbox::oci::Spec) -> bool {
        // check if the entrypoint of the spec is a wasm binary.
        let (module_name, _method) = oci::get_module(spec);
        let module_name = match module_name {
            Some(m) => m,
            None => {
                log::info!("wasmer cannot handle this workload, no arguments provided");
                return false;
            }
        };
        let path = PathBuf::from(module_name);

        // TODO: do we need to validate the wasm binary?
        // ```rust
        //   let bytes = std::fs::read(path).unwrap();
        //   wasmparser::validate(&bytes).is_ok()
        // ```

        path.extension()
            .map(|ext| ext.to_ascii_lowercase())
            .is_some_and(|ext| ext == "wasm" || ext == "wat")
    }
}

impl WasmerExecutor {
    fn prepare(&self, spec: &Spec, args: &[String]) -> anyhow::Result<()> {
        // already in the cgroup
        let envs = oci_wasmer::env_to_wasi(spec);
        log::info!("setting up wasi");

        if let Some(stdin) = self.stdin {
            dup(STDIN_FILENO)?;
            dup2(stdin, STDIN_FILENO)?;
        }
        if let Some(stdout) = self.stdout {
            dup(STDOUT_FILENO)?;
            dup2(stdout, STDOUT_FILENO)?;
        }
        if let Some(stderr) = self.stderr {
            dup(STDERR_FILENO)?;
            dup2(stderr, STDERR_FILENO)?;
        }

        log::info!("wasi context ready");
        let (module_name, method) = oci::get_module(spec);
        let module_name = match module_name {
            Some(m) => m,
            None => {
                return Err(anyhow::format_err!(
                    "no module provided, cannot load module from file within container"
                ))
            }
        };

        log::info!("loading module from file {} ", module_name);
        let mut store = Store::new(self.engine.clone());
        let module = Module::from_file(&store, module_name)?;

        log::info!("Starting `tokio` runtime...");
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let _guard = runtime.enter();

        log::info!("Creating `WasiEnv`.... args: {:?}, envs: {:?}", args, envs);
        let mut wasi_env = WasiEnv::builder(&method)
            // unknown command: /wasi-demo-app.wasm
            .args(args[1..].to_vec())
            .envs(envs)
            .finalize(&mut store)?;

        log::info!("Instantiating module with WASI imports...");
        let import_object = wasi_env.import_object(&mut store, &module)?;
        let instance = Instance::new(&mut store, &module, &import_object)?;

        wasi_env.initialize(&mut store, instance.clone())?;

        log::info!("Call WASI `_start` function...");
        let start_func = instance.exports.get_function(&method)?;
        start_func.call(&mut store, &[])?;

        wasi_env.cleanup(&mut store, None);

        Ok(())
    }
}
