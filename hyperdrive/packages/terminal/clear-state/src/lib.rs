use hyperware_process_lib::{
    kernel_types::StateAction, println, script, Address, ProcessId, Request,
};

wit_bindgen::generate!({
    path: "../target/wit",
    world: "process-v1",
});

const USAGE: &str = "\x1b[1mUsage:\x1b[0m clear-state <process-id>";
const STATE_PROCESS_ID: (&str, &str, &str) = ("state", "distro", "sys");

script!(init);
fn init(_our: Address, args: String) -> String {
    if args.is_empty() {
        return format!("Clear the state of the given process.\n{USAGE}");
    }

    let Ok(ref process_id) = args.parse::<ProcessId>() else {
        return format!(
            "'{args}' is not a process-id (e.g. `process-name:package-name:publisher.os`)\n{USAGE}"
        );
    };

    let Ok(Ok(_)) = Request::to(("our", STATE_PROCESS_ID))
        .body(serde_json::to_vec(&StateAction::DeleteState(process_id.clone())).unwrap())
        .send_and_await_response(5)
    else {
        return format!("Failed to delete state for process {process_id}");
    };

    format!("Deleted state of process {process_id}")
}
