use kinode_process_lib::{
    await_next_request_body, call_init, println, Address, Message, NodeId, Request,
};
use serde::{Deserialize, Serialize};

wit_bindgen::generate!({
    path: "wit",
    world: "process",
    exports: {
        world: Component,
    },
});

// types copied from runtime networking core

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Identity {
    pub name: NodeId,
    pub networking_key: String,
    pub ws_routing: Option<(String, u16)>,
    pub allowed_routers: Vec<NodeId>,
}

/// Must be parsed from message pack vector.
/// all Get actions must be sent from local process. used for debugging
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NetAction {
    /// get a list of peers we are connected to
    GetPeers,
    /// get the [`Identity`] struct for a single peer
    GetPeer(String),
    /// get the [`NodeId`] associated with a given namehash, if any
    GetName(String),
    /// get a user-readable diagnostics string containing networking inforamtion
    GetDiagnostics,
}

/// For now, only sent in response to a ConnectionRequest.
/// Must be parsed from message pack vector
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NetResponse {
    Accepted(NodeId),
    Rejected(NodeId),
    /// response to [`NetAction::GetPeers`]
    Peers(Vec<Identity>),
    /// response to [`NetAction::GetPeer`]
    Peer(Option<Identity>),
    /// response to [`NetAction::GetName`]
    Name(Option<String>),
    /// response to [`NetAction::GetDiagnostics`]. A user-readable string.
    Diagnostics(String),
}

call_init!(init);

fn init(_our: Address) {
    let Ok(args) = await_next_request_body() else {
        println!("failed to get args");
        return;
    };
    let Ok(name) = String::from_utf8(args) else {
        println!("argument must be a string");
        return;
    };
    let Ok(Ok(Message::Response { body, .. })) = Request::to(("our", "net", "distro", "sys"))
        .body(rmp_serde::to_vec(&NetAction::GetPeer(name.clone())).unwrap())
        .send_and_await_response(5)
    else {
        println!("failed to get response from networking module");
        return;
    };
    let Ok(NetResponse::Peer(maybe_peer_id)) = rmp_serde::from_slice(&body) else {
        println!("got malformed response from networking module");
        return;
    };
    match maybe_peer_id {
        Some(peer_id) => println!(
            "peer identity for {}:\n    networking key: {}\n    routing: {:?}\n    routers: {:?}",
            peer_id.name, peer_id.networking_key, peer_id.ws_routing, peer_id.allowed_routers
        ),
        None => println!("no PKI entry found with name {name}"),
    }
}
