use crate::net::types::{IdentityExt, NetData, Peer};
use crate::net::{tcp, utils, ws};
use lib::types::core::{Identity, KernelMessage, NodeRouting};
use rand::prelude::SliceRandom;
use tokio::sync::mpsc;

/// if target is a peer, queue to be routed
/// otherwise, create peer and initiate routing
pub async fn send_to_peer(ext: &IdentityExt, data: &NetData, km: KernelMessage) {
    if let Some(peer) = data.peers.get_mut(&km.target.node) {
        peer.sender.send(km).expect("net: peer sender was dropped");
    } else {
        let Some(peer_id) = data.pki.get(&km.target.node) else {
            return utils::error_offline(km, &ext.network_error_tx).await;
        };
        let (peer_tx, peer_rx) = mpsc::unbounded_channel();
        // send message to be routed
        peer_tx.send(km).unwrap();
        data.peers.insert(
            peer_id.name.clone(),
            Peer {
                identity: peer_id.clone(),
                routing_for: false,
                sender: peer_tx.clone(),
            },
        );
        tokio::spawn(connect_to_peer(
            ext.clone(),
            data.clone(),
            peer_id.clone(),
            peer_rx,
        ));
    }
}

/// based on peer's identity, either use one of their
/// protocols to connect directly, or loop through their
/// routers to open a passthroughconnection for us
///
/// if we fail to connect, remove the peer from the map
/// and return an offline error for each message in the receiver
async fn connect_to_peer(
    ext: IdentityExt,
    data: NetData,
    peer_id: Identity,
    peer_rx: mpsc::UnboundedReceiver<KernelMessage>,
) {
    if peer_id.is_direct() {
        utils::print_debug(
            &ext.print_tx,
            &format!("net: attempting to connect to {} directly", peer_id.name),
        )
        .await;
        if let Some((_ip, port)) = peer_id.tcp_routing() {
            match tcp::init_direct(&ext, &data, &peer_id, *port, false, peer_rx).await {
                Ok(()) => {
                    utils::print_debug(
                        &ext.print_tx,
                        &format!("net: connected to {} directly", peer_id.name),
                    )
                    .await;
                    return;
                }
                Err(peer_rx) => {
                    return handle_failed_connection(&ext, &data, &peer_id, peer_rx).await;
                }
            }
        }
        if let Some((_ip, port)) = peer_id.ws_routing() {
            match ws::init_direct(&ext, &data, &peer_id, *port, false, peer_rx).await {
                Ok(()) => {
                    utils::print_debug(
                        &ext.print_tx,
                        &format!("net: connected to {} directly", peer_id.name),
                    )
                    .await;
                    return;
                }
                Err(peer_rx) => {
                    return handle_failed_connection(&ext, &data, &peer_id, peer_rx).await;
                }
            }
        }
    } else {
        connect_via_router(&ext, &data, &peer_id, peer_rx).await;
    }
}

/// loop through the peer's routers, attempting to connect
async fn connect_via_router(
    ext: &IdentityExt,
    data: &NetData,
    peer_id: &Identity,
    mut peer_rx: mpsc::UnboundedReceiver<KernelMessage>,
) {
    let routers_shuffled = {
        let mut routers = match peer_id.routing {
            NodeRouting::Routers(ref routers) => routers.clone(),
            _ => vec![],
        };
        routers.shuffle(&mut rand::thread_rng());
        routers
    };
    for router_name in &routers_shuffled {
        if router_name.as_ref() == ext.our.name {
            // we can't route through ourselves
            continue;
        }
        let router_id = match data.pki.get(router_name.as_str()) {
            None => continue,
            Some(id) => id.clone(),
        };
        if let Some((_ip, port)) = router_id.tcp_routing() {
            match tcp::init_routed(ext, data, &peer_id, &router_id, *port, peer_rx).await {
                Ok(()) => {
                    utils::print_debug(
                        &ext.print_tx,
                        &format!("net: connected to {}", router_id.name),
                    )
                    .await;
                    return;
                }
                Err(e) => {
                    peer_rx = e;
                    continue;
                }
            }
        }
        if let Some((_ip, port)) = router_id.ws_routing() {
            match ws::init_routed(ext, data, &peer_id, &router_id, *port, peer_rx).await {
                Ok(()) => {
                    utils::print_debug(
                        &ext.print_tx,
                        &format!("net: connected to {}", router_id.name),
                    )
                    .await;
                    return;
                }
                Err(e) => {
                    peer_rx = e;
                    continue;
                }
            }
        }
    }
    handle_failed_connection(ext, data, &peer_id, peer_rx).await;
}

pub async fn handle_failed_connection(
    ext: &IdentityExt,
    data: &NetData,
    peer_id: &Identity,
    mut peer_rx: mpsc::UnboundedReceiver<KernelMessage>,
) {
    utils::print_debug(
        &ext.print_tx,
        &format!("net: failed to connect to {}", peer_id.name),
    )
    .await;
    drop(data.peers.remove(&peer_id.name));
    peer_rx.close();
    while let Some(km) = peer_rx.recv().await {
        utils::error_offline(km, &ext.network_error_tx).await;
    }
}
