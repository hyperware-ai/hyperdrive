use crate::hyperware::process::homepage;
use hyperware_process_lib::{
    await_message, call_init, get_blob,
    http::{self, server},
    println, Address, Capability, LazyLoadBlob, ProcessId, Request, Response,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Fetching OS version from main package
const CARGO_TOML: &str = include_str!("../../../../Cargo.toml");

const DEFAULT_FAVES: &[&str] = &["main:app-store:sys", "settings:settings:sys"];

type PersistedAppOrder = HashMap<String, u32>;

// Push notification subscription structure
#[derive(Serialize, Deserialize, Clone, Debug)]
struct PushSubscription {
    endpoint: String,
    keys: SubscriptionKeys,
    created_at: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct SubscriptionKeys {
    p256dh: String,
    auth: String,
}

// Notification actions for IPC with notifications server
#[derive(Serialize, Deserialize, Debug)]
enum NotificationsAction {
    SendNotification {
        title: String,
        body: String,
        icon: Option<String>,
        data: Option<serde_json::Value>,
    },
    GetPublicKey,
    InitializeKeys,
    AddSubscription {
        subscription: PushSubscription,
    },
    RemoveSubscription {
        endpoint: String,
    },
    ClearSubscriptions,
    GetSubscription {
        endpoint: String,
    },
}

#[derive(Serialize, Deserialize, Debug)]
enum NotificationsResponse {
    NotificationSent,
    PublicKey(String),
    KeysInitialized,
    SubscriptionAdded,
    SubscriptionRemoved,
    SubscriptionsCleared,
    SubscriptionInfo(Option<PushSubscription>),
    Err(String),
}

wit_bindgen::generate!({
    path: "../target/wit",
    world: "homepage-sys-v1",
    generate_unused_types: true,
    additional_derives: [serde::Deserialize, serde::Serialize],
});

call_init!(init);
fn init(our: Address) {
    println!("started");

    let mut app_data: BTreeMap<String, homepage::App> = BTreeMap::new();

    let mut http_server = server::HttpServer::new(5);
    let http_config = server::HttpBindingConfig::default();

    http_server
        .serve_ui("ui", vec!["/"], http_config.clone())
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
    let stylesheet = hyperware_process_lib::vfs::File {
        path: "/homepage:sys/pkg/persisted-hyperware.css".to_string(),
        timeout: 5,
    }
    .read()
    .unwrap_or_else(|_| {
        hyperware_process_lib::vfs::File {
            path: "/homepage:sys/pkg/hyperware.css".to_string(),
            timeout: 5,
        }
        .read()
        .expect("failed to get hyperware.css")
    });

    // save the stylesheet to the persisted file
    hyperware_process_lib::vfs::File {
        path: "/homepage:sys/pkg/persisted-hyperware.css".to_string(),
        timeout: 5,
    }
    .write(&stylesheet)
    .expect("failed to write to /persisted-hyperware.css");

    http_server
        .bind_http_static_path(
            "/hyperware.css",
            false, // hyperware.css is not auth'd so that apps on subdomains can use it too!
            false,
            Some("text/css".to_string()),
            stylesheet,
        )
        .expect("failed to bind /hyperware.css");

    http_server
        .bind_http_static_path(
            "/hyperware.svg",
            false, // hyperware.svg is not auth'd so that apps on subdomains can use it too!
            false,
            Some("image/svg+xml".to_string()),
            include_str!("../../pkg/hyperware.svg").into(),
        )
        .expect("failed to bind /hyperware.svg");

    http_server
        .bind_http_static_path(
            "/manifest.json",
            false, // manifest.json is not auth'd so that PWA works properly
            false,
            Some("application/json".to_string()),
            include_str!("../../pkg/ui/manifest.json").into(),
        )
        .expect("failed to bind /manifest.json");

    http_server
        .bind_http_static_path(
            "/ClashDisplay-Variable.woff2",
            false, // icon-180.png is not auth'd so that PWA works properly
            false,
            Some("font/woff2".to_string()),
            include_bytes!("../../pkg/ui/ClashDisplay-Variable.woff2").into(),
        )
        .expect("failed to bind /ClashDisplay-Variable.woff2");

    http_server
        .bind_http_static_path(
            "/NHaasGroteskTXPro-55Rg.woff",
            false, // icon-180.png is not auth'd so that PWA works properly
            false,
            Some("font/woff".to_string()),
            include_bytes!("../../pkg/ui/NHaasGroteskTXPro-55Rg.woff").into(),
        )
        .expect("failed to bind /NHaasGroteskTXPro-55Rg.woff");

    http_server
        .bind_http_static_path(
            "/Logomark%20Iris.svg",
            false, // Logomark Iris.svg is not auth'd so that PWA works properly
            false,
            Some("image/svg+xml".to_string()),
            include_str!("../../pkg/ui/Logomark Iris.svg").into(),
        )
        .expect("failed to bind /Logomark Iris.svg");

    http_server
        .bind_http_static_path(
            "/Logo%20Iris.svg",
            false, // Logo Iris.svg is not auth'd so that PWA works properly
            false,
            Some("image/svg+xml".to_string()),
            include_str!("../../pkg/ui/Logo Iris.svg").into(),
        )
        .expect("failed to bind /Logo Iris.svg");

    http_server
        .bind_http_static_path(
            "/NHaasGroteskTXPro-75Bd.woff",
            false, // icon-180.png is not auth'd so that PWA works properly
            false,
            Some("font/woff".to_string()),
            include_bytes!("../../pkg/ui/NHaasGroteskTXPro-75Bd.woff").into(),
        )
        .expect("failed to bind /NHaasGroteskTXPro-75Bd.woff");

    http_server
        .bind_http_static_path(
            "/chaneyextended.woff2",
            false, // icon-180.png is not auth'd so that PWA works properly
            false,
            Some("font/woff".to_string()),
            include_bytes!("../../pkg/ui/chaneyextended.woff2").into(),
        )
        .expect("failed to bind /chaneyextended.woff2");

    http_server
        .bind_http_static_path(
            "/icon.svg",
            false, // icon.svg is not auth'd so that PWA works properly
            false,
            Some("image/svg+xml".to_string()),
            include_str!("../../pkg/ui/icon.svg").into(),
        )
        .expect("failed to bind /icon.svg");

    http_server
        .bind_http_static_path(
            "/icon-180.png",
            false, // icon-180.png is not auth'd so that PWA works properly
            false,
            Some("image/png".to_string()),
            include_bytes!("../../pkg/ui/icon-180.png").into(),
        )
        .expect("failed to bind /icon.svg");

    http_server
        .bind_http_static_path(
            "/chaneyextended.woff2",
            false, // icon-180.png is not auth'd so that PWA works properly
            false,
            Some("image/png".to_string()),
            include_bytes!("../../pkg/ui/chaneyextended.woff2").into(),
        )
        .expect("failed to bind /icon.svg");

    // because boot uses this path to check if homepage is served yet,
    // it's best to respond dynamically and only serve this path once
    // all of the apps/widgets have populated.
    http_server
        .bind_http_path("/version", http_config.clone())
        .expect("failed to bind /version");

    http_server
        .bind_http_path("/apps", http_config.clone())
        .expect("failed to bind /apps");
    http_server
        .bind_http_path("/favorite", http_config.clone())
        .expect("failed to bind /favorite");
    http_server
        .bind_http_path("/order", http_config.clone())
        .expect("failed to bind /order");

    // Notification endpoints
    http_server
        .bind_http_path("/api/notifications/vapid-key", http_config.clone())
        .expect("failed to bind /api/notifications/vapid-key");
    http_server
        .bind_http_path("/api/notifications/subscribe", http_config.clone())
        .expect("failed to bind /api/notifications/subscribe");
    http_server
        .bind_http_path("/api/notifications/unsubscribe", http_config.clone())
        .expect("failed to bind /api/notifications/unsubscribe");
    http_server
        .bind_http_path("/api/notifications/subscription-info", http_config.clone())
        .expect("failed to bind /api/notifications/subscription-info");
    http_server
        .bind_http_path("/api/notifications/unsubscribe-all", http_config.clone())
        .expect("failed to bind /api/notifications/unsubscribe-all");
    http_server
        .bind_http_path("/api/notifications/test-vapid", http_config)
        .expect("failed to bind /api/notifications/test-vapid");

    hyperware_process_lib::homepage::add_to_homepage(
        "Clock",
        None,
        None,
        Some(&make_clock_widget()),
    );

    // load persisted app order
    let mut persisted_app_order =
        hyperware_process_lib::get_typed_state(|bytes| serde_json::from_slice(bytes))
            .unwrap_or(PersistedAppOrder::new());

    // No longer loading push subscription from disk - it's now stored in notifications server

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
                                        &app_data.values().collect::<Vec<&homepage::App>>(),
                                    )
                                    .unwrap(),
                                )),
                            ),
                            "/version" => {
                                // hacky way to ensure that the homepage has populated itself before
                                // loading in after boot
                                if app_data.len() >= 4
                                    && app_data.values().filter(|app| app.widget.is_some()).count()
                                        >= 3
                                {
                                    (
                                        server::HttpResponse::new(http::StatusCode::OK),
                                        Some(LazyLoadBlob::new(
                                            Some("text/plain"),
                                            version_from_cargo_toml().as_bytes().to_vec(),
                                        )),
                                    )
                                } else {
                                    (server::HttpResponse::new(http::StatusCode::TOO_EARLY), None)
                                }
                            }
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
                                hyperware_process_lib::set_state(
                                    &serde_json::to_vec(&persisted_app_order).unwrap(),
                                );
                                (server::HttpResponse::new(http::StatusCode::OK), None)
                            }
                            "/api/notifications/vapid-key" => {
                                // Get VAPID public key from notifications server
                                let notifications_address = Address::new(
                                    &our.node,
                                    ProcessId::new(Some("notifications"), "distro", "sys"),
                                );

                                match Request::to(notifications_address)
                                    .body(
                                        serde_json::to_vec(&NotificationsAction::GetPublicKey)
                                            .unwrap(),
                                    )
                                    .send_and_await_response(5)
                                {
                                    Ok(Ok(response)) => {
                                        let response_body = response.body();

                                        // Try to deserialize and log the result
                                        match serde_json::from_slice::<NotificationsResponse>(response_body) {
                                            Ok(NotificationsResponse::PublicKey(key)) => {
                                                (
                                                    server::HttpResponse::new(http::StatusCode::OK),
                                                    Some(LazyLoadBlob::new(
                                                        Some("application/json"),
                                                        serde_json::to_vec(&serde_json::json!({
                                                            "publicKey": key
                                                        }))
                                                        .unwrap(),
                                                    )),
                                                )
                                            }
                                            Ok(other) => {
                                                println!("homepage: unexpected response type: {:?}", other);
                                                (
                                                    server::HttpResponse::new(http::StatusCode::INTERNAL_SERVER_ERROR),
                                                    Some(LazyLoadBlob::new(
                                                        Some("application/json"),
                                                        serde_json::to_vec(&serde_json::json!({
                                                            "error": "Unexpected response from notifications service"
                                                        }))
                                                        .unwrap(),
                                                    )),
                                                )
                                            }
                                            Err(e) => {
                                                println!("homepage: failed to deserialize response: {}", e);
                                                (
                                                    server::HttpResponse::new(http::StatusCode::INTERNAL_SERVER_ERROR),
                                                    Some(LazyLoadBlob::new(
                                                        Some("application/json"),
                                                        serde_json::to_vec(&serde_json::json!({
                                                            "error": format!("Failed to parse response: {}", e)
                                                        }))
                                                        .unwrap(),
                                                    )),
                                                )
                                            }
                                        }
                                    }
                                    Ok(Err(e)) => {
                                        println!("homepage: notifications module returned error: {:?}", e);
                                        (
                                            server::HttpResponse::new(
                                                http::StatusCode::INTERNAL_SERVER_ERROR,
                                            ),
                                            Some(LazyLoadBlob::new(
                                                Some("application/json"),
                                                serde_json::to_vec(&serde_json::json!({
                                                    "error": "Failed to get public key"
                                                }))
                                                .unwrap(),
                                            )),
                                        )
                                    }
                                    _ => (
                                        server::HttpResponse::new(
                                            http::StatusCode::INTERNAL_SERVER_ERROR,
                                        ),
                                        Some(LazyLoadBlob::new(
                                            Some("application/json"),
                                            serde_json::to_vec(&serde_json::json!({
                                                "error": "Failed to contact notifications server"
                                            }))
                                            .unwrap(),
                                        )),
                                    ),
                                }
                            }
                            "/api/notifications/subscribe" => {
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
                                let Ok(subscription) =
                                    serde_json::from_slice::<PushSubscription>(&body.bytes)
                                else {
                                    return (
                                        server::HttpResponse::new(http::StatusCode::BAD_REQUEST),
                                        None,
                                    );
                                };

                                // Send subscription to notifications server
                                let notifications_address = Address::new(
                                    &our.node,
                                    ProcessId::new(Some("notifications"), "distro", "sys"),
                                );

                                match Request::to(notifications_address)
                                    .body(
                                        serde_json::to_vec(&NotificationsAction::AddSubscription {
                                            subscription,
                                        })
                                        .unwrap(),
                                    )
                                    .send_and_await_response(5)
                                {
                                    Ok(Ok(response)) => {
                                        match serde_json::from_slice::<NotificationsResponse>(response.body()) {
                                            Ok(NotificationsResponse::SubscriptionAdded) => {
                                                (
                                                    server::HttpResponse::new(http::StatusCode::OK),
                                                    Some(LazyLoadBlob::new(
                                                        Some("application/json"),
                                                        serde_json::to_vec(&serde_json::json!({
                                                            "success": true
                                                        }))
                                                        .unwrap(),
                                                    )),
                                                )
                                            }
                                            Ok(NotificationsResponse::Err(e)) => {
                                                println!("homepage: notifications server error: {}", e);
                                                (
                                                    server::HttpResponse::new(http::StatusCode::INTERNAL_SERVER_ERROR),
                                                    Some(LazyLoadBlob::new(
                                                        Some("application/json"),
                                                        serde_json::to_vec(&serde_json::json!({
                                                            "error": format!("Failed to add subscription: {}", e)
                                                        }))
                                                        .unwrap(),
                                                    )),
                                                )
                                            }
                                            _ => {
                                                (
                                                    server::HttpResponse::new(http::StatusCode::INTERNAL_SERVER_ERROR),
                                                    Some(LazyLoadBlob::new(
                                                        Some("application/json"),
                                                        serde_json::to_vec(&serde_json::json!({
                                                            "error": "Unexpected response from notifications service"
                                                        }))
                                                        .unwrap(),
                                                    )),
                                                )
                                            }
                                        }
                                    }
                                    _ => (
                                        server::HttpResponse::new(http::StatusCode::INTERNAL_SERVER_ERROR),
                                        Some(LazyLoadBlob::new(
                                            Some("application/json"),
                                            serde_json::to_vec(&serde_json::json!({
                                                "error": "Failed to contact notifications server"
                                            }))
                                            .unwrap(),
                                        )),
                                    ),
                                }
                            }
                            "/api/notifications/unsubscribe" => {
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

                                // Parse the endpoint from the request body
                                let Ok(request_data) = serde_json::from_slice::<serde_json::Value>(&body.bytes) else {
                                    return (
                                        server::HttpResponse::new(http::StatusCode::BAD_REQUEST),
                                        None,
                                    );
                                };

                                let Some(endpoint) = request_data.get("endpoint").and_then(|e| e.as_str()) else {
                                    return (
                                        server::HttpResponse::new(http::StatusCode::BAD_REQUEST),
                                        Some(LazyLoadBlob::new(
                                            Some("application/json"),
                                            serde_json::to_vec(&serde_json::json!({
                                                "error": "Missing endpoint in request body"
                                            }))
                                            .unwrap(),
                                        )),
                                    );
                                };

                                // Remove specific subscription from notifications server
                                let notifications_address = Address::new(
                                    &our.node,
                                    ProcessId::new(Some("notifications"), "distro", "sys"),
                                );

                                match Request::to(notifications_address)
                                    .body(
                                        serde_json::to_vec(&NotificationsAction::RemoveSubscription {
                                            endpoint: endpoint.to_string(),
                                        })
                                        .unwrap(),
                                    )
                                    .send_and_await_response(5)
                                {
                                    Ok(Ok(response)) => {
                                        match serde_json::from_slice::<NotificationsResponse>(response.body()) {
                                            Ok(NotificationsResponse::SubscriptionRemoved) => {
                                                (
                                                    server::HttpResponse::new(http::StatusCode::OK),
                                                    Some(LazyLoadBlob::new(
                                                        Some("application/json"),
                                                        serde_json::to_vec(&serde_json::json!({
                                                            "success": true
                                                        }))
                                                        .unwrap(),
                                                    )),
                                                )
                                            }
                                            Ok(NotificationsResponse::Err(e)) => {
                                                println!("homepage: notifications server error: {}", e);
                                                (
                                                    server::HttpResponse::new(http::StatusCode::INTERNAL_SERVER_ERROR),
                                                    Some(LazyLoadBlob::new(
                                                        Some("application/json"),
                                                        serde_json::to_vec(&serde_json::json!({
                                                            "error": format!("Failed to remove subscription: {}", e)
                                                        }))
                                                        .unwrap(),
                                                    )),
                                                )
                                            }
                                            _ => (
                                                server::HttpResponse::new(http::StatusCode::INTERNAL_SERVER_ERROR),
                                                Some(LazyLoadBlob::new(
                                                    Some("application/json"),
                                                    serde_json::to_vec(&serde_json::json!({
                                                        "error": "Unexpected response from notifications service"
                                                    }))
                                                    .unwrap(),
                                                )),
                                            ),
                                        }
                                    }
                                    _ => (
                                        server::HttpResponse::new(http::StatusCode::INTERNAL_SERVER_ERROR),
                                        Some(LazyLoadBlob::new(
                                            Some("application/json"),
                                            serde_json::to_vec(&serde_json::json!({
                                                "error": "Failed to contact notifications server"
                                            }))
                                            .unwrap(),
                                        )),
                                    ),
                                }
                            }
                            "/api/notifications/subscription-info" => {
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

                                // Parse the endpoint from the request body
                                let Ok(request_data) = serde_json::from_slice::<serde_json::Value>(&body.bytes) else {
                                    return (
                                        server::HttpResponse::new(http::StatusCode::BAD_REQUEST),
                                        None,
                                    );
                                };

                                let Some(endpoint) = request_data.get("endpoint").and_then(|e| e.as_str()) else {
                                    return (
                                        server::HttpResponse::new(http::StatusCode::BAD_REQUEST),
                                        Some(LazyLoadBlob::new(
                                            Some("application/json"),
                                            serde_json::to_vec(&serde_json::json!({
                                                "error": "Missing endpoint in request body"
                                            }))
                                            .unwrap(),
                                        )),
                                    );
                                };

                                // Get subscription info from notifications server
                                let notifications_address = Address::new(
                                    &our.node,
                                    ProcessId::new(Some("notifications"), "distro", "sys"),
                                );

                                match Request::to(notifications_address)
                                    .body(
                                        serde_json::to_vec(&NotificationsAction::GetSubscription {
                                            endpoint: endpoint.to_string(),
                                        })
                                        .unwrap(),
                                    )
                                    .send_and_await_response(5)
                                {
                                    Ok(Ok(response)) => {
                                        match serde_json::from_slice::<NotificationsResponse>(response.body()) {
                                            Ok(NotificationsResponse::SubscriptionInfo(sub_option)) => {
                                                (
                                                    server::HttpResponse::new(http::StatusCode::OK),
                                                    Some(LazyLoadBlob::new(
                                                        Some("application/json"),
                                                        serde_json::to_vec(&serde_json::json!({
                                                            "subscription": sub_option
                                                        }))
                                                        .unwrap(),
                                                    )),
                                                )
                                            }
                                            Ok(NotificationsResponse::Err(e)) => {
                                                println!("homepage: notifications server error: {}", e);
                                                (
                                                    server::HttpResponse::new(http::StatusCode::INTERNAL_SERVER_ERROR),
                                                    Some(LazyLoadBlob::new(
                                                        Some("application/json"),
                                                        serde_json::to_vec(&serde_json::json!({
                                                            "error": format!("Failed to get subscription info: {}", e)
                                                        }))
                                                        .unwrap(),
                                                    )),
                                                )
                                            }
                                            _ => (
                                                server::HttpResponse::new(http::StatusCode::INTERNAL_SERVER_ERROR),
                                                Some(LazyLoadBlob::new(
                                                    Some("application/json"),
                                                    serde_json::to_vec(&serde_json::json!({
                                                        "error": "Unexpected response from notifications service"
                                                    }))
                                                    .unwrap(),
                                                )),
                                            ),
                                        }
                                    }
                                    _ => (
                                        server::HttpResponse::new(http::StatusCode::INTERNAL_SERVER_ERROR),
                                        Some(LazyLoadBlob::new(
                                            Some("application/json"),
                                            serde_json::to_vec(&serde_json::json!({
                                                "error": "Failed to contact notifications server"
                                            }))
                                            .unwrap(),
                                        )),
                                    ),
                                }
                            }
                            "/api/notifications/unsubscribe-all" => {
                                // Clear all subscriptions from notifications server
                                let notifications_address = Address::new(
                                    &our.node,
                                    ProcessId::new(Some("notifications"), "distro", "sys"),
                                );

                                match Request::to(notifications_address)
                                    .body(
                                        serde_json::to_vec(&NotificationsAction::ClearSubscriptions)
                                        .unwrap(),
                                    )
                                    .send_and_await_response(5)
                                {
                                    Ok(Ok(response)) => {
                                        match serde_json::from_slice::<NotificationsResponse>(response.body()) {
                                            Ok(NotificationsResponse::SubscriptionsCleared) => {
                                                (
                                                    server::HttpResponse::new(http::StatusCode::OK),
                                                    Some(LazyLoadBlob::new(
                                                        Some("application/json"),
                                                        serde_json::to_vec(&serde_json::json!({
                                                            "success": true
                                                        }))
                                                        .unwrap(),
                                                    )),
                                                )
                                            }
                                            _ => (
                                                server::HttpResponse::new(http::StatusCode::INTERNAL_SERVER_ERROR),
                                                Some(LazyLoadBlob::new(
                                                    Some("application/json"),
                                                    serde_json::to_vec(&serde_json::json!({
                                                        "error": "Failed to clear subscriptions"
                                                    }))
                                                    .unwrap(),
                                                )),
                                            ),
                                        }
                                    }
                                    _ => (
                                        server::HttpResponse::new(http::StatusCode::INTERNAL_SERVER_ERROR),
                                        Some(LazyLoadBlob::new(
                                            Some("application/json"),
                                            serde_json::to_vec(&serde_json::json!({
                                                "error": "Failed to contact notifications server"
                                            }))
                                            .unwrap(),
                                        )),
                                    ),
                                }
                            }
                            "/api/notifications/test-vapid" => {

                                // Get VAPID public key from notifications server
                                let notifications_address = Address::new(
                                    &our.node,
                                    ProcessId::new(Some("notifications"), "distro", "sys"),
                                );

                                match Request::to(notifications_address)
                                    .body(
                                        serde_json::to_vec(&NotificationsAction::GetPublicKey)
                                            .unwrap(),
                                    )
                                    .send_and_await_response(5)
                                {
                                    Ok(Ok(response)) => {
                                        match serde_json::from_slice::<NotificationsResponse>(response.body()) {
                                            Ok(NotificationsResponse::PublicKey(key)) => {
                                                // Return key info for client-side validation
                                                let key_info = serde_json::json!({
                                                    "publicKey": key,
                                                    "keyLength": key.len(),
                                                    "note": "Decode this key client-side to validate format"
                                                });

                                                (
                                                    server::HttpResponse::new(http::StatusCode::OK),
                                                    Some(LazyLoadBlob::new(
                                                        Some("application/json"),
                                                        serde_json::to_vec(&key_info).unwrap(),
                                                    )),
                                                )
                                            }
                                            _ => (
                                                server::HttpResponse::new(http::StatusCode::INTERNAL_SERVER_ERROR),
                                                Some(LazyLoadBlob::new(
                                                    Some("application/json"),
                                                    serde_json::to_vec(&serde_json::json!({
                                                        "error": "Unexpected response from notifications service"
                                                    }))
                                                    .unwrap(),
                                                )),
                                            ),
                                        }
                                    }
                                    _ => (
                                        server::HttpResponse::new(http::StatusCode::INTERNAL_SERVER_ERROR),
                                        Some(LazyLoadBlob::new(
                                            Some("application/json"),
                                            serde_json::to_vec(&serde_json::json!({
                                                "error": "Failed to contact notifications server"
                                            }))
                                            .unwrap(),
                                        )),
                                    ),
                                }
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
            // handle messages to get apps, add or remove an app from the homepage.
            // they must have messaging access to us in order to perform this.
            if let Ok(request) = serde_json::from_slice::<homepage::Request>(message.body()) {
                match request {
                    homepage::Request::Add(homepage::AddRequest {
                        label,
                        icon,
                        path,
                        widget,
                    }) => {
                        let id = message.source().process.to_string();
                        app_data.insert(
                            id.clone(),
                            homepage::App {
                                id: id.clone(),
                                process: message.source().process().to_string(),
                                package_name: message.source().package().to_string(),
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
                    homepage::Request::Remove => {
                        let id = message.source().process.to_string();
                        app_data.remove(&id);
                        persisted_app_order.remove(&id);
                    }
                    homepage::Request::RemoveOther(id) => {
                        // caps check
                        let required_capability = Capability::new(
                            &our,
                            serde_json::to_string(&homepage::Capability::RemoveOther).unwrap(),
                        );
                        if !message.capabilities().contains(&required_capability) {
                            continue;
                        }
                        // end caps check
                        app_data.remove(&id);
                        persisted_app_order.remove(&id);
                    }
                    homepage::Request::GetApps => {
                        let apps = app_data.values().cloned().collect::<Vec<homepage::App>>();
                        let resp = homepage::Response::GetApps(apps);
                        Response::new()
                            .body(serde_json::to_vec(&resp).unwrap())
                            .send()
                            .unwrap();
                    }
                    homepage::Request::GetPushSubscription => {
                        // Subscriptions are no longer stored in homepage - they're in the notifications server
                        // Return None to indicate no subscription available from homepage
                        let resp = homepage::Response::PushSubscription(None);
                        Response::new()
                            .body(serde_json::to_vec(&resp).unwrap())
                            .send()
                            .unwrap();
                    }
                    homepage::Request::SetStylesheet(new_stylesheet_string) => {
                        // caps check
                        let required_capability = Capability::new(
                            &our,
                            serde_json::to_string(&homepage::Capability::SetStylesheet).unwrap(),
                        );
                        if !message.capabilities().contains(&required_capability) {
                            continue;
                        }
                        // end caps check
                        hyperware_process_lib::vfs::File {
                            path: "/homepage:sys/pkg/persisted-hyperware.css".to_string(),
                            timeout: 5,
                        }
                        .write(new_stylesheet_string.as_bytes())
                        .expect("failed to write to /persisted-hyperware.css");
                        // re-bind
                        http_server
                            .bind_http_static_path(
                                "/hyperware.css",
                                false, // hyperware.css is not auth'd so that apps on subdomains can use it too!
                                false,
                                Some("text/css".to_string()),
                                new_stylesheet_string.into(),
                            )
                            .expect("failed to bind /hyperware.css");
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

fn make_clock_widget() -> String {
    return format!(
        r#"<html>
    <head>
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <link rel="stylesheet" href="/hyperware.css">
        <style>
            .clock {{
                width: 200px;
                height: 200px;
                border-radius: 50%;
                position: relative;
                margin: 20px auto;
                background: black;
            }}
            .hand {{
                position: absolute;
                bottom: 50%;
                left: 50%;
                transform-origin: bottom;
                background-color: white;
            }}
            .hour {{
                width: 4px;
                height: 60px;
                margin-left: -2px;
            }}
            .minute {{
                width: 3px;
                height: 80px;
                margin-left: -1.5px;
            }}
            .second {{
                width: 2px;
                height: 90px;
                margin-left: -1px;
                background: #ff0000;
            }}
            .center {{
                width: 12px;
                height: 12px;
                border-radius: 50%;
                position: absolute;
                top: 50%;
                left: 50%;
                transform: translate(-50%, -50%);
                background-color: white;
            }}
            .marker {{
                position: absolute;
                width: 2px;
                height: 4px;
                left: 50%;
                margin-left: -1px;
                transform-origin: 50% 100px;
                background: white;
            }}
            .marker.primary {{
                width: 3px;
                height: 8px;
                margin-left: -1.5px;
            }}
            .digital-time {{
                font-family: var(--font-family-main);
                margin-top: 1em;
                font-size: 0.7em;
                color: light-dark(black, white);
                position: absolute;
                left: 50%;
                transform: translateX(-50%);
                width: fit-content;
                text-align: center;
                bottom: 40px;
                background-color: white;
                padding: 0.25em 0.5em;
                border-radius: 0.5em;
                max-width: fit-content;
            }}
            @media (prefers-color-scheme: dark) {{
                body {{
                    background-color: #000;
                }}
                .clock {{
                    background: white;
                    border: white;
                }}
                .hand.hour,
                .hand.minute  {{
                    background-color: black;
                }}
                .marker {{
                    background-color: black;
                }}
                .center {{
                    background-color: black;
                }}
                .digital-time {{
                    background-color: black;
                }}
            }}
        </style>
    </head>
    <body style="margin: 0; overflow: hidden;" >
        <div class="clock">
            <div class="marker primary" style="transform: rotate(0deg)"></div>
            <div class="marker" style="transform: rotate(30deg)"></div>
            <div class="marker" style="transform: rotate(60deg)"></div>
            <div class="marker primary" style="transform: rotate(90deg)"></div>
            <div class="marker" style="transform: rotate(120deg)"></div>
            <div class="marker" style="transform: rotate(150deg)"></div>
            <div class="marker primary" style="transform: rotate(180deg)"></div>
            <div class="marker" style="transform: rotate(210deg)"></div>
            <div class="marker" style="transform: rotate(240deg)"></div>
            <div class="marker primary" style="transform: rotate(270deg)"></div>
            <div class="marker" style="transform: rotate(300deg)"></div>
            <div class="marker" style="transform: rotate(330deg)"></div>
            <div class="hand hour" id="hour"></div>
            <div class="hand minute" id="minute"></div>
            <div class="hand second" id="second"></div>
            <div class="center"></div>
        </div>
        <div class="digital-time" id="digital"></div>

        <script>
            function updateClock() {{
                const now = new Date();
                const hours = now.getHours() % 12;
                const minutes = now.getMinutes();
                const seconds = now.getSeconds();

                const hourDeg = (hours * 30) + (minutes * 0.5);
                const minuteDeg = minutes * 6;
                const secondDeg = seconds * 6;

                document.getElementById('hour').style.transform = `rotate(${{hourDeg}}deg)`;
                document.getElementById('minute').style.transform = `rotate(${{minuteDeg}}deg)`;
                document.getElementById('second').style.transform = `rotate(${{secondDeg}}deg)`;

                // Update digital display
                const displayHours = hours === 0 ? 12 : hours;
                const displayMinutes = minutes.toString().padStart(2, '0');
                document.getElementById('digital').textContent = `${{displayHours}}:${{displayMinutes}}`;
            }}

            setInterval(updateClock, 1000);
            updateClock();
        </script>
    </body>
</html>"#
    );
}
