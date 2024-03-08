use clap::{Arg, Command};
use kinode_process_lib::{
    await_next_request_body, call_init, println, Address, Request, SendErrorKind,
};
use regex::Regex;

wit_bindgen::generate!({
    path: "wit",
    world: "process",
    exports: {
        world: Component,
    },
});

call_init!(init);

fn init(_our: Address) {
    let Ok(body) = await_next_request_body() else {
        println!("failed to get args");
        return;
    };
    let body_string = String::from_utf8(body).unwrap();
    if body_string.is_empty() {
        println!("Send a Request to a Process");
        println!("\x1b[1mUsage:\x1b[0m m <target> <body> [-a <await_time>]");
        return;
    }

    let re = Regex::new(r#"'[^']*'|\S+"#).unwrap();
    let mut args: Vec<String> = re
        .find_iter(body_string.as_str())
        .map(|mat| {
            let match_str = mat.as_str();
            // Remove the surrounding single quotes for the JSON string
            if match_str.starts_with('\'') && match_str.ends_with('\'') {
                match_str[1..match_str.len() - 1].to_string()
            } else {
                match_str.to_string()
            }
        })
        .collect();

    args.insert(0, "m".to_string());

    let Ok(parsed) = Command::new("m")
        .disable_help_flag(true)
        .arg(Arg::new("target").index(1).required(true))
        .arg(Arg::new("body").index(2).required(true))
        .arg(
            Arg::new("await")
                .short('a')
                .long("await")
                .value_parser(clap::value_parser!(u64)),
        )
        .try_get_matches_from(args)
    else {
        println!("failed to parse args");
        return;
    };

    let Some(target) = parsed.get_one::<String>("target") else {
        println!("no target");
        return;
    };

    let Ok(target) = target.parse::<Address>() else {
        println!("invalid address: \"{target}\"");
        return;
    };

    let Some(body) = parsed.get_one::<String>("body") else {
        println!("no body");
        return;
    };

    let req = Request::new().target(target).body(body.as_bytes().to_vec());

    match parsed.get_one::<u64>("await") {
        Some(s) => {
            println!("awaiting response for {}s", s);
            match req.send_and_await_response(*s).unwrap() {
                Ok(res) => {
                    println!("{}", String::from_utf8(res.body().to_vec()).unwrap());
                }
                Err(e) => {
                    println!(
                        "{}",
                        match e.kind {
                            SendErrorKind::Timeout =>
                                "target did not send Response in time, try increasing the await time",
                            SendErrorKind::Offline =>
                                "failed to send message because the target is offline",
                        }
                    );
                }
            }
        }
        None => {
            let _ = req.send().unwrap();
        }
    }
}
