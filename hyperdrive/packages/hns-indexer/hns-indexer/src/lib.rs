use crate::hyperware::process::hns_indexer::{
    IndexerRequest, IndexerResponse, NamehashToNameRequest, NodeInfoRequest, ResetError,
    ResetResult, WitHnsUpdate, WitState,
};
use alloy_primitives::keccak256;
use alloy_sol_types::SolEvent;
use hyperware::process::standard::clear_state;
use hyperware_process_lib::logging::{debug, error, info, init_logging, warn, Level};
use hyperware_process_lib::{
    await_message, call_init, eth, get_state, hypermap, net, set_state, timer, Address, Capability,
    Message, Request, Response,
};
use std::{
    collections::{BTreeMap, HashMap},
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    str::FromStr,
};

wit_bindgen::generate!({
    path: "target/wit",
    world: "hns-indexer-sys-v0",
    generate_unused_types: true,
    additional_derives: [serde::Deserialize, serde::Serialize, process_macros::SerdeJsonInto],
});

const MAX_PENDING_ATTEMPTS: u8 = 5;
const SUBSCRIPTION_TIMEOUT_S: u64 = 60;
// 2s:
//  - compare to src/eth/mod.rs DELAY_MS of 1_000: long enough to invalidate cache
//  - ~ block time on Base
const DELAY_MS: u64 = 2_000;
const CHECKPOINT_MS: u64 = 5 * 60 * 1_000; // 5 minutes

type PendingNotes = BTreeMap<u64, Vec<(String, String, eth::Bytes, u8)>>;

#[derive(serde::Serialize, serde::Deserialize)]
struct State {
    /// the chain id we are indexing
    chain_id: u64,
    /// what contract this state pertains to
    contract_address: eth::Address,
    /// namehash to human readable name
    names: HashMap<String, String>,
    /// human readable name to most recent on-chain routing information as json
    nodes: HashMap<String, net::HnsUpdate>,
    /// last saved checkpoint block
    last_checkpoint_block: u64,
}

impl State {
    fn load() -> Option<Self> {
        let Some(ref state_bytes) = get_state() else {
            return None;
        };

        rmp_serde::from_slice(state_bytes).ok()
    }
}

#[derive(Clone, serde::Serialize)]
struct StateV1 {
    /// namehash to human readable name
    names: HashMap<String, String>,
    /// human readable name to most recent on-chain routing information as json
    nodes: HashMap<String, net::HnsUpdate>,
    /// last saved checkpoint block
    last_block: u64,
    #[serde(skip)]
    hypermap: hypermap::Hypermap,
    /// notes are under a mint; in case they come out of order, store till next block
    // #[serde(skip)] // TODO: ?
    pending_notes: PendingNotes,
    is_checkpoint_timer_live: bool,
}

impl<'de> serde::Deserialize<'de> for StateV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Create a helper struct without the hypermap field
        #[derive(serde::Deserialize)]
        struct StateHelper {
            names: HashMap<String, String>,
            nodes: HashMap<String, net::HnsUpdate>,
            last_block: u64,
            pending_notes: PendingNotes,
        }

        let helper = StateHelper::deserialize(deserializer)?;

        Ok(StateV1 {
            names: helper.names,
            nodes: helper.nodes,
            last_block: helper.last_block,
            hypermap: hypermap::Hypermap::default(SUBSCRIPTION_TIMEOUT_S),
            pending_notes: helper.pending_notes,
            is_checkpoint_timer_live: false,
        })
    }
}

impl StateV1 {
    fn new() -> Self {
        StateV1 {
            names: HashMap::new(),
            nodes: HashMap::new(),
            last_block: hypermap::HYPERMAP_FIRST_BLOCK,
            hypermap: hypermap::Hypermap::default(SUBSCRIPTION_TIMEOUT_S),
            pending_notes: BTreeMap::new(),
            is_checkpoint_timer_live: false,
        }
    }

    fn load() -> Self {
        match get_state() {
            None => Self::new(),
            Some(state_bytes) => match rmp_serde::from_slice(&state_bytes) {
                Ok(state) => state,
                Err(e) => {
                    warn!("failed to deserialize saved state: {e:?}");
                    Self::new()
                }
            },
        }
    }

    /// Reset by removing the checkpoint and reloading fresh state
    fn reset(&self) {
        clear_state();
    }

    /// Saves a checkpoint, serializes to the current block
    fn save(&mut self) {
        match rmp_serde::to_vec(self) {
            Ok(state_bytes) => set_state(&state_bytes),
            Err(e) => error!("failed to serialize state: {e:?}"),
        }
    }

    /// loops through saved nodes, and sends them to net
    /// called upon bootup
    fn send_nodes(&self) -> anyhow::Result<()> {
        for node in self.nodes.values() {
            Request::to(("our", "net", "distro", "sys"))
                .body(rmp_serde::to_vec(&net::NetAction::HnsUpdate(node.clone()))?)
                .send()?;
        }
        Ok(())
    }

    fn subscribe(&self) {
        let (mints_filter, notes_filter) = make_filters(None);

        self.hypermap.provider.subscribe_loop(1, mints_filter, 1, 0);
        self.hypermap.provider.subscribe_loop(2, notes_filter, 1, 0);
    }

    fn fetch_and_process_logs(&mut self) {
        let (mints_filter, notes_filter) = make_filters(None);

        self.fetch_and_process_logs_filter(mints_filter);
        self.fetch_and_process_logs_filter(notes_filter);
    }

    /// Get logs for a filter then process them while taking pending notes into account.
    fn fetch_and_process_logs_filter(&mut self, filter: eth::Filter) {
        loop {
            match self.hypermap.provider.get_logs(&filter) {
                Ok(logs) => {
                    debug!("log len: {}", logs.len());
                    for log in logs {
                        if let Err(e) = self.handle_log(&log) {
                            error!("log-handling error! {e:?}");
                        }
                    }
                    return;
                }
                Err(e) => {
                    error!("got eth error while fetching logs: {e:?}, trying again in 5s...");
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
            }
        }
    }

    fn handle_eth_message(&mut self, body: &[u8]) -> anyhow::Result<()> {
        match serde_json::from_slice::<eth::EthSubResult>(body) {
            Ok(Ok(eth::EthSub { result, .. })) => {
                if let Ok(eth::SubscriptionResult::Log(log)) =
                    serde_json::from_value::<eth::SubscriptionResult>(result)
                {
                    if let Err(e) = self.handle_log(&log) {
                        error!("log-handling error! {e:?}");
                    }
                }
            }
            Ok(Err(e)) => {
                error!("got eth subscription error ({e:?}), resubscribing");
                let (mints_filter, notes_filter) = make_filters(None);
                if e.id == 1 {
                    self.hypermap.provider.subscribe_loop(1, mints_filter, 2, 0);
                } else if e.id == 2 {
                    self.hypermap.provider.subscribe_loop(2, notes_filter, 2, 0);
                }
            }
            Err(e) => {
                error!("failed to deserialize message from eth:distro:sys: {e:?}")
            }
        }

        self.handle_pending_notes()?;

        Ok(())
    }

    fn handle_log(&mut self, log: &eth::Log) -> anyhow::Result<()> {
        if let Some(block) = log.block_number {
            self.last_block = block;
        }

        match log.topics()[0] {
            hypermap::contract::Mint::SIGNATURE_HASH => {
                let decoded = hypermap::contract::Mint::decode_log_data(log.data(), true).unwrap();
                let parent_hash = decoded.parenthash.to_string();
                let child_hash = decoded.childhash.to_string();
                let name = String::from_utf8(decoded.label.to_vec())?;

                self.add_mint(&parent_hash, &child_hash, &name)?;
            }
            hypermap::contract::Note::SIGNATURE_HASH => {
                let (decoded, parent_hash, note) = decode_note(log)?;

                self.add_note(&parent_hash, &note, &decoded.data, log.block_number, 0)?;
            }
            _ => {}
        };

        Ok(())
    }

    fn add_mint(&mut self, parent_hash: &str, child_hash: &str, name: &str) -> anyhow::Result<()> {
        if !hypermap::valid_name(&name) {
            return Err(anyhow::anyhow!("skipping invalid name: {name}"));
        }

        let full_name = match self.names.get(parent_hash) {
            Some(parent_name) => &format!("{name}.{parent_name}"),
            None => name,
        };
        debug!("mint {full_name}");

        self.names
            .insert(child_hash.to_string(), full_name.to_string());
        self.nodes.insert(
            full_name.to_string(),
            net::HnsUpdate {
                name: full_name.to_string(),
                public_key: String::new(),
                ips: Vec::new(),
                ports: BTreeMap::new(),
                routers: Vec::new(),
            },
        );

        Ok(())
    }

    fn add_note(
        &mut self,
        parent_hash: &str,
        note: &str,
        data: &eth::Bytes,
        block_number: Option<u64>,
        attempt_number: u8,
    ) -> anyhow::Result<()> {
        if !hypermap::valid_note(note) {
            return Err(anyhow::anyhow!("skipping invalid note: {note}"));
        }

        let Some(parent_name) = self.names.get(parent_hash) else {
            if let Some(block_number) = block_number {
                self.pending_notes.entry(block_number).or_default().push((
                    parent_hash.to_string(),
                    note.to_string(),
                    data.clone(),
                    attempt_number,
                ));
                debug!("note put into pending: {note}");
            } else {
                error!("note should go into pending, but no block_number given. dropping note. parent_hash, note: {parent_hash}, {note}");
            }
            return Ok(());
        };
        debug!("note {parent_name}: {note}");

        match note {
            "~ws-port" => {
                let ws = bytes_to_port(data)?;
                if let Some(node) = self.nodes.get_mut(parent_name) {
                    node.ports.insert("ws".to_string(), ws);
                    // port defined, -> direct
                    node.routers = vec![];
                }
            }
            "~tcp-port" => {
                let tcp = bytes_to_port(data)?;
                if let Some(node) = self.nodes.get_mut(parent_name) {
                    node.ports.insert("tcp".to_string(), tcp);
                    // port defined, -> direct
                    node.routers = vec![];
                }
            }
            "~net-key" => {
                if data.len() != 32 {
                    return Err(anyhow::anyhow!("invalid net-key length"));
                }
                if let Some(node) = self.nodes.get_mut(parent_name) {
                    node.public_key = hex::encode(data);
                }
            }
            "~routers" => {
                let routers = self.decode_routers(data);
                if let Some(node) = self.nodes.get_mut(parent_name) {
                    node.routers = routers;
                    // -> indirect
                    node.ports = BTreeMap::new();
                    node.ips = vec![];
                }
            }
            "~ip" => {
                let ip = bytes_to_ip(data)?;
                if let Some(node) = self.nodes.get_mut(parent_name) {
                    node.ips = vec![ip.to_string()];
                    // -> direct
                    node.routers = vec![];
                }
            }
            _other => {
                // Ignore unknown notes
            }
        }

        // only send an update if we have a *full* set of data for networking:
        // a node name, plus either <routers> or <ip, port(s)>
        if let Some(node_info) = self.nodes.get(parent_name) {
            if !node_info.public_key.is_empty()
                && ((!node_info.ips.is_empty() && !node_info.ports.is_empty())
                    || node_info.routers.len() > 0)
            {
                Request::to(("our", "net", "distro", "sys"))
                    .body(rmp_serde::to_vec(&net::NetAction::HnsUpdate(
                        node_info.clone(),
                    ))?)
                    .send()?;
            }
        }

        Ok(())
    }

    fn handle_pending_notes(&mut self) -> anyhow::Result<()> {
        if self.pending_notes.is_empty() {
            return Ok(());
        }

        // walk through pending_notes
        //  - add_note() ones that are ripe
        //  - push unripe back into pending_notes
        let pending_notes = std::mem::take(&mut self.pending_notes);
        for (block, notes) in pending_notes {
            if block >= self.last_block {
                // not ripe yet: push back into pending_notes
                self.pending_notes.insert(block, notes);
            } else {
                // ripe: call add_note()
                for (parent_hash, note, data, attempt) in notes.iter() {
                    if attempt >= &MAX_PENDING_ATTEMPTS {
                        error!("pending note exceeded max attempts; dropping: parent_hash, note: {parent_hash}, {note}");
                        continue;
                    }
                    self.add_note(parent_hash, note, data, Some(block), attempt + 1)?;
                }
            }
        }

        if !self.pending_notes.is_empty() {
            timer::set_timer(DELAY_MS, None);
        }

        Ok(())
    }

    fn handle_tick(&mut self, is_checkpoint: bool) -> anyhow::Result<()> {
        let block_number = self.hypermap.provider.get_block_number();
        if let Ok(block_number) = block_number {
            debug!("new block: {block_number}");
            self.last_block = block_number;
            if is_checkpoint {
                self.is_checkpoint_timer_live = false;
                self.save();
            }
        }

        self.handle_pending_notes()?;

        Ok(())
    }

    /// Decodes bytes under ~routers in hypermap into an array of keccak256 hashes (32 bytes each)
    /// and returns the associated node identities.
    fn decode_routers(&self, data: &[u8]) -> Vec<String> {
        if data.len() % 32 != 0 {
            warn!("got invalid data length for router hashes: {}", data.len());
            return vec![];
        }

        let mut routers = Vec::new();
        for chunk in data.chunks(32) {
            let hash_str = format!("0x{}", hex::encode(chunk));

            match self.names.get(&hash_str) {
                Some(full_name) => routers.push(full_name.clone()),
                None => error!("no name found for router hash {hash_str}"),
            }
        }

        routers
    }

    pub fn fetch_node(&self, timeout: &u64, name: &str) -> Option<net::HnsUpdate> {
        let hypermap = hypermap::Hypermap::default(timeout - 1);
        if let Ok((_tba, _owner, _data)) = hypermap.get(name) {
            let Ok(Some(public_key_bytes)) = hypermap
                .get(&format!("~net-key.{name}"))
                .map(|(_, _, data)| data)
            else {
                return None;
            };

            let maybe_ip = hypermap
                .get(&format!("~ip.{name}"))
                .map(|(_, _, data)| data.map(|b| bytes_to_ip(&b)));

            let maybe_tcp_port = hypermap
                .get(&format!("~tcp-port.{name}"))
                .map(|(_, _, data)| data.map(|b| bytes_to_port(&b)));

            let maybe_ws_port = hypermap
                .get(&format!("~ws-port.{name}"))
                .map(|(_, _, data)| data.map(|b| bytes_to_port(&b)));

            let maybe_routers = hypermap
                .get(&format!("~routers.{name}"))
                .map(|(_, _, data)| data.map(|b| self.decode_routers(&b)));

            let mut ports = BTreeMap::new();
            if let Ok(Some(Ok(tcp_port))) = maybe_tcp_port {
                ports.insert("tcp".to_string(), tcp_port);
            }
            if let Ok(Some(Ok(ws_port))) = maybe_ws_port {
                ports.insert("ws".to_string(), ws_port);
            }

            Some(net::HnsUpdate {
                name: name.to_string(),
                public_key: hex::encode(public_key_bytes),
                ips: if let Ok(Some(Ok(ip))) = maybe_ip {
                    vec![ip.to_string()]
                } else {
                    vec![]
                },
                ports,
                routers: if let Ok(Some(routers)) = maybe_routers {
                    routers
                } else {
                    vec![]
                },
            })
        } else {
            None
        }
    }

    fn handle_indexer_request(&mut self, our: &Address, message: Message) -> anyhow::Result<bool> {
        let mut is_reset = false;

        let Message::Request {
            ref expects_response,
            ..
        } = message
        else {
            return Err(anyhow::anyhow!(
                "got Response input to handle_indexer_request"
            ));
        };

        let response_body = match message.body().try_into()? {
            IndexerRequest::NamehashToName(NamehashToNameRequest { ref hash, .. }) => {
                // TODO: make sure we've seen the whole block, while actually
                // sending a response to the proper place.
                IndexerResponse::Name(self.names.get(hash).cloned())
            }
            IndexerRequest::NodeInfo(NodeInfoRequest { ref name, .. }) => {
                match self.nodes.get(name) {
                    Some(node) => IndexerResponse::NodeInfo(Some(node.clone().into())),
                    None => {
                        // we don't have the node in our state: try hypermap.get()
                        //  to see if it exists onchain and the indexer missed it
                        let mut response = IndexerResponse::NodeInfo(None);
                        if let Some(timeout) = expects_response {
                            if let Some(hns_update) = self.fetch_node(timeout, name) {
                                response =
                                    IndexerResponse::NodeInfo(Some(hns_update.clone().into()));
                                // save the node to state
                                self.nodes.insert(name.clone(), hns_update.clone());
                                // produce namehash and save in names map
                                self.names.insert(hypermap::namehash(name), name.clone());
                                // send the node to net
                                Request::to(("our", "net", "distro", "sys"))
                                    .body(rmp_serde::to_vec(&net::NetAction::HnsUpdate(
                                        hns_update,
                                    ))?)
                                    .send()?;
                            }
                        }
                        response
                    }
                }
            }
            IndexerRequest::Reset => {
                // check for root capability
                let root_cap = Capability::new(our.clone(), "{\"root\":true}");
                if message.source().package_id() != our.package_id()
                    || !message.capabilities().contains(&root_cap)
                {
                    IndexerResponse::Reset(ResetResult::Err(ResetError::NoRootCap))
                } else {
                    // reload state fresh - this will create new db
                    info!("resetting state");
                    self.reset();
                    is_reset = true;
                    IndexerResponse::Reset(ResetResult::Success)
                }
            }
            IndexerRequest::GetState(_) => IndexerResponse::GetState(self.clone().into()),
        };

        if expects_response.is_some() {
            Response::new().body(response_body).send()?;
        }

        Ok(is_reset)
    }
}

fn decode_note(note_log: &eth::Log) -> anyhow::Result<(hypermap::contract::Note, String, String)> {
    let decoded = hypermap::contract::Note::decode_log_data(note_log.data(), true).unwrap();
    let parent_hash = decoded.parenthash.to_string();
    let note = String::from_utf8(decoded.label.to_vec())?;
    Ok((decoded, parent_hash, note))
}

impl From<State> for StateV1 {
    fn from(old: State) -> Self {
        StateV1 {
            names: old.names,
            nodes: old.nodes,
            last_block: old.last_checkpoint_block,
            hypermap: hypermap::Hypermap::default(SUBSCRIPTION_TIMEOUT_S),
            pending_notes: BTreeMap::new(),
            is_checkpoint_timer_live: false,
        }
    }
}

impl From<net::HnsUpdate> for WitHnsUpdate {
    fn from(k: net::HnsUpdate) -> Self {
        WitHnsUpdate {
            name: k.name,
            public_key: k.public_key,
            ips: k.ips,
            ports: k.ports.into_iter().map(|(k, v)| (k, v)).collect::<Vec<_>>(),
            routers: k.routers,
        }
    }
}

impl From<WitHnsUpdate> for net::HnsUpdate {
    fn from(k: WitHnsUpdate) -> Self {
        net::HnsUpdate {
            name: k.name,
            public_key: k.public_key,
            ips: k.ips,
            ports: BTreeMap::from_iter(k.ports),
            routers: k.routers,
        }
    }
}

impl From<StateV1> for WitState {
    fn from(s: StateV1) -> Self {
        // TODO: store this
        let contract_address: [u8; 20] = hypermap::HYPERMAP_ADDRESS.as_bytes().try_into().unwrap();
        let contract_address: Vec<u8> = contract_address.into();

        WitState {
            chain_id: hypermap::HYPERMAP_CHAIN_ID,
            contract_address,
            names: s.names.into_iter().map(|(k, v)| (k, v)).collect::<Vec<_>>(),
            nodes: s
                .nodes
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect::<Vec<_>>(),
            last_block: s.last_block,
        }
    }
}

fn make_filters(from_block: Option<u64>) -> (eth::Filter, eth::Filter) {
    let hypermap_address = eth::Address::from_str(hypermap::HYPERMAP_ADDRESS).unwrap();
    let from_block = from_block.unwrap_or_else(|| hypermap::HYPERMAP_FIRST_BLOCK);
    // sub_id: 1
    // listen to all mint events in hypermap
    let mints_filter = eth::Filter::new()
        .address(hypermap_address)
        .from_block(from_block)
        .to_block(eth::BlockNumberOrTag::Latest)
        .event(hypermap::contract::Mint::SIGNATURE);

    // sub_id: 2
    // listen to all note events that are relevant to the HNS protocol within hypermap
    let notes_filter = eth::Filter::new()
        .address(hypermap_address)
        .from_block(from_block)
        .to_block(eth::BlockNumberOrTag::Latest)
        .event(hypermap::contract::Note::SIGNATURE)
        .topic3(vec![
            keccak256("~ws-port"),
            keccak256("~tcp-port"),
            keccak256("~net-key"),
            keccak256("~routers"),
            keccak256("~ip"),
        ]);

    (mints_filter, notes_filter)
}

// TEMP. Either remove when event reimitting working with anvil,
// or refactor into better structure(!)
#[cfg(feature = "simulation-mode")]
fn add_temp_hardcoded_tlzs(state: &mut StateV1) {
    // add some hardcoded top level zones
    state.names.insert(
        "0xdeeac81ae11b64e7cab86d089c306e5d223552a630f02633ce170d2786ff1bbd".to_string(),
        "os".to_string(),
    );
    state.names.insert(
        "0x137d9e4cc0479164d40577620cb3b41b083c6e8dbf58f8523be76d207d6fd8ea".to_string(),
        "dev".to_string(),
    );
}

/// convert IP address stored at ~ip in hypermap to IpAddr
pub fn bytes_to_ip(bytes: &[u8]) -> anyhow::Result<IpAddr> {
    match bytes.len() {
        4 => {
            // IPv4 address
            let ip_num = u32::from_be_bytes(bytes.try_into().unwrap());
            Ok(IpAddr::V4(Ipv4Addr::from(ip_num)))
        }
        16 => {
            // IPv6 address
            let ip_num = u128::from_be_bytes(bytes.try_into().unwrap());
            Ok(IpAddr::V6(Ipv6Addr::from(ip_num)))
        }
        _ => Err(anyhow::anyhow!("Invalid byte length for IP address")),
    }
}

/// convert port stored at ~[protocol]-port in hypermap to u16
pub fn bytes_to_port(bytes: &[u8]) -> anyhow::Result<u16> {
    match bytes.len() {
        2 => Ok(u16::from_be_bytes([bytes[0], bytes[1]])),
        _ => Err(anyhow::anyhow!("Invalid byte length for port")),
    }
}

fn main(our: &Address, state: &mut StateV1) -> anyhow::Result<()> {
    #[cfg(feature = "simulation-mode")]
    add_temp_hardcoded_tlzs(state);

    // loop through checkpointed values and send to net
    if let Err(e) = state.send_nodes() {
        error!("failed to send nodes to net: {e}");
    }

    state.subscribe();

    // if block in state is < current_block, get logs from that part.
    info!("syncing old logs from block: {}", state.last_block);

    state.fetch_and_process_logs();

    // set a timer tick so any pending logs will be processed
    timer::set_timer(DELAY_MS, None);

    // set a timer tick for checkpointing
    timer::set_timer(CHECKPOINT_MS, Some(b"checkpoint".to_vec()));

    debug!("done syncing old logs");

    let mut is_checkpoint = false;
    let mut is_reset = false;

    loop {
        let Ok(message) = await_message() else {
            continue;
        };

        if !message.is_request() {
            // only expect to hear Response from timer
            if message.is_local() && message.source().process == "timer:distro:sys" {
                is_checkpoint = message.context() == Some(b"checkpoint");
                state.handle_tick(is_checkpoint)?;
            }
        } else {
            if message.is_local() && message.source().process == "eth:distro:sys" {
                state.handle_eth_message(message.body())?;
            } else {
                is_reset = state.handle_indexer_request(our, message)?;
            }
        }

        if state.pending_notes.is_empty() && !state.is_checkpoint_timer_live && !is_checkpoint {
            // set checkpoint timer
            debug!("set checkpoint timer");
            state.is_checkpoint_timer_live = true;
            timer::set_timer(CHECKPOINT_MS, Some(b"checkpoint".to_vec()));
        }

        if is_reset {
            // reset state works like:
            //  1. call `state.reset()` to clear state on disk (happens in
            //     `handle_indexer_request()`
            //  2. return from `main()`, causing `init()` to loop
            //     and call `main()` again
            //  3. `main()` attempts to load state from disk, sees it is empty,
            //     and loads it from scratch from the chain
            return Ok(());
        }
    }
}

call_init!(init);
fn init(our: Address) {
    init_logging(Level::DEBUG, Level::INFO, None, None, None).unwrap();
    info!("begin");

    // state is checkpointed regularly (default every 5 minutes if new events are found)
    //
    // to maintain backwards compatibility, try loading old version of state first
    //  (and convert to current version)
    let mut state: StateV1 = if let Some(old) = State::load() {
        old.into()
    } else {
        StateV1::load()
    };

    loop {
        if let Err(e) = main(&our, &mut state) {
            error!("fatal error: {e}");
            break;
        }
    }
}
