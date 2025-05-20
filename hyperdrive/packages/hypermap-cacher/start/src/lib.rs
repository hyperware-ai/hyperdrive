use hyperware_process_lib::{
    call_init, get_capability, spawn, Address, Capability, OnExit, ProcessId,
};

wit_bindgen::generate!({
    path: "target/wit",
    world: "process-v1",
    generate_unused_types: false,
    additional_derives: [],
});

const PROCESS_NAME: &str = "hypermap-cacher";

fn to_messaging_cap(our: &Address, process: &str) -> Capability {
    Capability {
        issuer: Address::new(&our.node, process.parse::<ProcessId>().unwrap()),
        params: "\"messaging\"".into(),
    }
}

fn to_messaging_tuple(process: &str) -> (ProcessId, String) {
    (
        process.parse::<ProcessId>().unwrap(),
        "\"messaging\"".to_string(),
    )
}

call_init!(init);
fn init(our: Address) {
    let correspondents = vec![
        "eth:distro:sys",
        "http-server:distro:sys",
        "sign:sign:sys",
        "timer:distro:sys",
        "vfs:distro:sys",
    ];

    let networking = get_capability(
        &Address::from(("our", "kernel", "distro", "sys")),
        "\"network\"",
    )
    .expect("couldn't get networking capability");

    let mut caps: Vec<Capability> = correspondents
        .iter()
        .map(|process| to_messaging_cap(&our, process))
        .collect();
    caps.push(networking);

    let _spawned_process_id = match spawn(
        Some(PROCESS_NAME),
        &format!("{}/pkg/{PROCESS_NAME}.wasm", our.package_id()),
        OnExit::Restart,
        caps,
        correspondents
            .iter()
            .map(|process| to_messaging_tuple(process))
            .collect(),
        false,
    ) {
        Ok(_) => {}
        Err(e) => panic!("couldn't spawn {PROCESS_NAME}: {e:?}"),
    };
}
