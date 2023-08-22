use containerd_shim_wasm::sandbox::oci::Spec;

pub fn env_to_wasi(spec: &Spec) -> Vec<(String, String)> {
    let default = vec![];
    let env = spec
        .process()
        .as_ref()
        .unwrap()
        .env()
        .as_ref()
        .unwrap_or(&default);
    let mut vec: Vec<(String, String)> = Vec::with_capacity(env.len());

    for v in env {
        match v.split_once('=') {
            None => vec.push((v.to_string(), "".to_string())),
            Some(t) => vec.push((t.0.to_string(), t.1.to_string())),
        };
    }

    vec
}
