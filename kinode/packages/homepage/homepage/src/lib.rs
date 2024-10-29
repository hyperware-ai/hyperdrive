use crate::kinode::process::homepage::{AddRequest, Request as HomepageRequest};
use kinode_process_lib::{
    await_message, call_init, get_blob, http, http::server, println, Address, LazyLoadBlob,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Fetching OS version from main package
const CARGO_TOML: &str = include_str!("../../../../Cargo.toml");

const DEFAULT_FAVES: &[&str] = &[
    "chess:chess:sys",
    "main:app-store:sys",
    "settings:settings:sys",
];

#[derive(Serialize, Deserialize)]
struct HomepageApp {
    id: String,
    process: String,
    package: String,
    publisher: String,
    path: Option<String>,
    label: String,
    base64_icon: Option<String>,
    widget: Option<String>,
    order: u32,
    favorite: bool, // **not currently used on frontend**
}

type PersistedAppOrder = HashMap<String, u32>;

wit_bindgen::generate!({
    path: "target/wit",
    world: "homepage-sys-v0",
    generate_unused_types: true,
    additional_derives: [serde::Deserialize, serde::Serialize],
});

call_init!(init);
fn init(our: Address) {
    println!("begin");

    let mut app_data: BTreeMap<String, HomepageApp> = BTreeMap::new();

    let mut http_server = server::HttpServer::new(5);
    let http_config = server::HttpBindingConfig::default();

    http_server
        .serve_ui(&our, "ui", vec!["/"], http_config.clone())
        .expect("failed to serve ui");

    http_server
        .bind_http_static_path(
            "/our",
            false,
            false,
            Some("text/html".to_string()),
            our.node().into(),
        )
        .expect("failed to bind to /our");

    http_server
        .bind_http_static_path(
            "/amionline",
            false,
            false,
            Some("text/html".to_string()),
            "yes".into(),
        )
        .expect("failed to bind to /amionline");

    http_server
        .bind_http_static_path(
            "/our.js",
            false,
            false,
            Some("application/javascript".to_string()),
            format!("window.our = {{}}; window.our.node = '{}';", &our.node).into(),
        )
        .expect("failed to bind to /our.js");

    // the base version gets written over on-bootstrap, so we look for
    // the persisted (user-customized) version first.
    // if it doesn't exist, we use the bootstrapped version and save it here.
    let stylesheet = kinode_process_lib::vfs::File {
        path: "/homepage:sys/pkg/persisted-kinode.css".to_string(),
        timeout: 5,
    }
    .read()
    .unwrap_or_else(|_| {
        kinode_process_lib::vfs::File {
            path: "/homepage:sys/pkg/kinode.css".to_string(),
            timeout: 5,
        }
        .read()
        .expect("failed to get kinode.css")
    });

    // save the stylesheet to the persisted file
    kinode_process_lib::vfs::File {
        path: "/homepage:sys/pkg/persisted-kinode.css".to_string(),
        timeout: 5,
    }
    .write(&stylesheet)
    .expect("failed to write to /persisted-kinode.css");

    http_server
        .bind_http_static_path(
            "/kinode.css",
            false, // kinode.css is not auth'd so that apps on subdomains can use it too!
            false,
            Some("text/css".to_string()),
            stylesheet,
        )
        .expect("failed to bind /kinode.css");

    http_server
        .bind_http_static_path(
            "/kinode.svg",
            false, // kinode.svg is not auth'd so that apps on subdomains can use it too!
            false,
            Some("image/svg+xml".to_string()),
            include_str!("../../pkg/kinode.svg").into(),
        )
        .expect("failed to bind /kinode.svg");

    http_server
        .bind_http_static_path(
            "/bird-orange.svg",
            false, // bird-orange.svg is not auth'd so that apps on subdomains can use it too!
            false,
            Some("image/svg+xml".to_string()),
            include_str!("../../pkg/bird-orange.svg").into(),
        )
        .expect("failed to bind /bird-orange.svg");

    http_server
        .bind_http_static_path(
            "/bird-plain.svg",
            false, // bird-plain.svg is not auth'd so that apps on subdomains can use it too!
            false,
            Some("image/svg+xml".to_string()),
            include_str!("../../pkg/bird-plain.svg").into(),
        )
        .expect("failed to bind /bird-plain.svg");

    http_server
        .bind_http_static_path(
            "/version",
            true,
            false,
            Some("text/plain".to_string()),
            version_from_cargo_toml().into(),
        )
        .expect("failed to bind /version");

    http_server
        .bind_http_path("/apps", http_config.clone())
        .expect("failed to bind /apps");
    http_server
        .bind_http_path("/favorite", http_config.clone())
        .expect("failed to bind /favorite");
    http_server
        .bind_http_path("/order", http_config)
        .expect("failed to bind /order");

    // load persisted app order
    let mut persisted_app_order =
        kinode_process_lib::get_typed_state(|bytes| serde_json::from_slice(bytes))
            .unwrap_or(PersistedAppOrder::new());

    loop {
        let Ok(ref message) = await_message() else {
            // we never send requests, so this will never happen
            continue;
        };
        if message.source().process == "http-server:distro:sys" {
            if message.is_request() {
                let Ok(request) = http_server.parse_request(message.body()) else {
                    continue;
                };
                http_server.handle_request(
                    request,
                    |incoming| {
                        let path = incoming.bound_path(None);
                        match path {
                            "/apps" => (
                                server::HttpResponse::new(http::StatusCode::OK),
                                Some(LazyLoadBlob::new(
                                    Some("application/json"),
                                    serde_json::to_vec(
                                        &app_data.values().collect::<Vec<&HomepageApp>>(),
                                    )
                                    .unwrap(),
                                )),
                            ),
                            "/favorite" => {
                                let Ok(http::Method::POST) = incoming.method() else {
                                    return (
                                        server::HttpResponse::new(
                                            http::StatusCode::METHOD_NOT_ALLOWED,
                                        ),
                                        None,
                                    );
                                };
                                let Some(body) = get_blob() else {
                                    return (
                                        server::HttpResponse::new(http::StatusCode::BAD_REQUEST),
                                        None,
                                    );
                                };
                                let Ok(favorite_toggle) =
                                    serde_json::from_slice::<(String, bool)>(&body.bytes)
                                else {
                                    return (
                                        server::HttpResponse::new(http::StatusCode::BAD_REQUEST),
                                        None,
                                    );
                                };
                                if let Some(app) = app_data.get_mut(&favorite_toggle.0) {
                                    app.favorite = favorite_toggle.1;
                                }
                                (server::HttpResponse::new(http::StatusCode::OK), None)
                            }
                            "/order" => {
                                let Ok(http::Method::POST) = incoming.method() else {
                                    return (
                                        server::HttpResponse::new(
                                            http::StatusCode::METHOD_NOT_ALLOWED,
                                        ),
                                        None,
                                    );
                                };
                                let Some(body) = get_blob() else {
                                    return (
                                        server::HttpResponse::new(http::StatusCode::BAD_REQUEST),
                                        None,
                                    );
                                };
                                let Ok(order_list) =
                                    serde_json::from_slice::<Vec<(String, u32)>>(&body.bytes)
                                else {
                                    return (
                                        server::HttpResponse::new(http::StatusCode::BAD_REQUEST),
                                        None,
                                    );
                                };
                                for (app_id, order) in &order_list {
                                    if let Some(app) = app_data.get_mut(app_id) {
                                        app.order = *order;
                                    }
                                }
                                persisted_app_order = order_list.into_iter().collect();
                                kinode_process_lib::set_state(
                                    &serde_json::to_vec(&persisted_app_order).unwrap(),
                                );
                                (server::HttpResponse::new(http::StatusCode::OK), None)
                            }
                            _ => (server::HttpResponse::new(http::StatusCode::NOT_FOUND), None),
                        }
                    },
                    |_channel_id, _message_type, _message| {
                        // not expecting any websocket messages from FE currently
                    },
                );
            }
        } else {
            // handle messages to add or remove an app from the homepage.
            // they must have messaging access to us in order to perform this.
            if let Ok(request) = serde_json::from_slice::<HomepageRequest>(message.body()) {
                match request {
                    HomepageRequest::Add(AddRequest {
                        label,
                        icon,
                        path,
                        widget,
                    }) => {
                        let id = message.source().process.to_string();
                        app_data.insert(
                            id.clone(),
                            HomepageApp {
                                id: id.clone(),
                                process: message.source().process().to_string(),
                                package: message.source().package().to_string(),
                                publisher: message.source().publisher().to_string(),
                                path: path.map(|path| {
                                    format!(
                                        "/{}/{}",
                                        message.source().process,
                                        path.strip_prefix('/').unwrap_or(&path)
                                    )
                                }),
                                label,
                                base64_icon: icon,
                                widget,
                                order: if let Some(order) = persisted_app_order.get(&id) {
                                    *order
                                } else {
                                    app_data.len() as u32
                                },
                                favorite: DEFAULT_FAVES
                                    .contains(&message.source().process.to_string().as_str()),
                            },
                        );
                    }
                    HomepageRequest::Remove => {
                        let id = message.source().process.to_string();
                        app_data.remove(&id);
                        persisted_app_order.remove(&id);
                    }
                    HomepageRequest::SetStylesheet(new_stylesheet_string) => {
                        // ONLY settings:settings:sys may call this request
                        if message.source().process != "settings:settings:sys" {
                            continue;
                        }
                        kinode_process_lib::vfs::File {
                            path: "/homepage:sys/pkg/persisted-kinode.css".to_string(),
                            timeout: 5,
                        }
                        .write(new_stylesheet_string.as_bytes())
                        .expect("failed to write to /persisted-kinode.css");
                        // re-bind
                        http_server
                            .bind_http_static_path(
                                "/kinode.css",
                                false, // kinode.css is not auth'd so that apps on subdomains can use it too!
                                false,
                                Some("text/css".to_string()),
                                new_stylesheet_string.into(),
                            )
                            .expect("failed to bind /kinode.css");
                        println!("updated kinode.css!");
                    }
                }
            }
        }
    }
}

fn version_from_cargo_toml() -> String {
    let version = CARGO_TOML
        .lines()
        .find(|line| line.starts_with("version = "))
        .expect("Failed to find version in Cargo.toml");

    version
        .split('=')
        .last()
        .expect("Failed to parse version from Cargo.toml")
        .trim()
        .trim_matches('"')
        .to_string()
}
