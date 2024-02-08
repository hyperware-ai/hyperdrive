use kinode_process_lib::{
    await_next_request_body, call_init, println, Address, ProcessId, Request,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

wit_bindgen::generate!({
    path: "wit",
    world: "process",
    exports: {
        world: Component,
    },
});

#[derive(Debug, Serialize, Deserialize)]
enum TerminalAction {
    EditAlias {
        alias: String,
        process: Option<ProcessId>,
    },
}

call_init!(init);

fn init(_our: Address) {
    let Ok(args) = await_next_request_body() else {
        println!("alias: failed to get args, aborting");
        return;
    };

    let line = String::from_utf8(args).unwrap_or("alias: error".into());
    if line.is_empty() {
        println!("Change alias for a process");
        println!("\x1b[1mUsage:\x1b[0m alias <alias_name> <process_id>");
        return;
    }

    let (alias, process) = line.split_once(" ").unwrap_or((&line, ""));

    if alias.is_empty() {
        println!("alias: no alias given");
        return;
    }

    if process.is_empty() {
        let _ = Request::new()
            .target(("our", "terminal", "terminal", "sys"))
            .body(
                json!(TerminalAction::EditAlias {
                    alias: alias.to_string(),
                    process: None
                })
                .to_string()
                .as_bytes()
                .to_vec(),
            )
            .send();
    } else {
        match process.parse::<ProcessId>() {
            Ok(process) => {
                let _ = Request::new()
                    .target(("our", "terminal", "terminal", "sys"))
                    .body(
                        json!(TerminalAction::EditAlias {
                            alias: alias.to_string(),
                            process: Some(process)
                        })
                        .to_string()
                        .as_bytes()
                        .to_vec(),
                    )
                    .send();
            }
            Err(_) => {
                println!("alias: invalid process id");
            }
        }
    }
}
