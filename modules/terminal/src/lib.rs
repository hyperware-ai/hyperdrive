cargo_component_bindings::generate!();
use bindings::{component::uq_process::types::*, print_to_terminal, receive, send_request, Guest};

#[allow(dead_code)]
mod process_lib;

struct Component;

fn parse_command(our_name: &str, line: &str) {
    let (head, tail) = line.split_once(" ").unwrap_or((&line, ""));
    match head {
        "" | " " => {}
        "!hi" => {
            let (target, message) = match tail.split_once(" ") {
                Some((s, t)) => (s, t),
                None => {
                    print_to_terminal(0, &format!("invalid command: \"{}\"", line));
                    return;
                }
            };
            send_request(
                &Address {
                    node: if target == "our" {
                        our_name.into()
                    } else {
                        target.into()
                    },
                    process: ProcessId::from_str("net:sys:uqbar").unwrap(),
                },
                &Request {
                    inherit: false,
                    expects_response: Some(5),
                    ipc: message.into(),
                    metadata: None,
                },
                None,
                None,
            );
        }
        "!message" => {
            let (target_node, tail) = match tail.split_once(" ") {
                Some((s, t)) => (s, t),
                None => {
                    print_to_terminal(0, &format!("invalid command: \"{}\"", line));
                    return;
                }
            };
            let (target_process, ipc) = match tail.split_once(" ") {
                Some((a, p)) => (a, p),
                None => {
                    print_to_terminal(0, &format!("invalid command: \"{}\"", line));
                    return;
                }
            };
            //  TODO: why does this work but using the API below does not?
            //        Is it related to passing json in rather than a Serialize type?
            //
            send_request(
                &Address {
                    node: if target_node == "our" {
                        our_name.into()
                    } else {
                        target_node.into()
                    },
                    process: ProcessId::from_str(target_process).unwrap_or_else(|_| {
                        ProcessId::from_str(&format!("{}:sys:uqbar", target_process)).unwrap()
                    }),
                },
                &Request {
                    inherit: false,
                    expects_response: None,
                    ipc: ipc.into(),
                    metadata: None,
                },
                None,
                None,
            );
        }
        _ => {
            print_to_terminal(0, &format!("invalid command: \"{line}\""));
        }
    }
}

impl Guest for Component {
    fn init(our: Address) {
        assert_eq!(our.process.to_string(), "terminal:terminal:uqbar");
        print_to_terminal(1, &format!("terminal: start"));
        loop {
            let (source, message) = match receive() {
                Ok((source, message)) => (source, message),
                Err((error, _context)) => {
                    print_to_terminal(0, &format!("net error: {:?}!", error.kind));
                    continue;
                }
            };
            match message {
                Message::Request(Request {
                    expects_response,
                    ipc,
                    ..
                }) => {
                    if our.node != source.node || our.process != source.process {
                        continue;
                    }
                    parse_command(&our.node, std::str::from_utf8(&ipc).unwrap_or_default());
                }
                Message::Response((Response { ipc, metadata, .. }, _)) => {
                    if let Ok(txt) = std::str::from_utf8(&ipc) {
                        print_to_terminal(0, &format!("net response: {}", txt));
                    }
                }
            }
        }
    }
}
