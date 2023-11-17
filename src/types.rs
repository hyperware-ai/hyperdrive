use crate::kernel::uqbar::process::standard as wit;
use ring::signature;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

lazy_static::lazy_static! {
    pub static ref ENCRYPTOR_PROCESS_ID: ProcessId = ProcessId::new(Some("encryptor"), "sys", "uqbar");
    pub static ref ETH_RPC_PROCESS_ID: ProcessId = ProcessId::new(Some("eth_rpc"), "sys", "uqbar");
    pub static ref FILESYSTEM_PROCESS_ID: ProcessId = ProcessId::new(Some("filesystem"), "sys", "uqbar");
    pub static ref HTTP_CLIENT_PROCESS_ID: ProcessId = ProcessId::new(Some("http_client"), "sys", "uqbar");
    pub static ref HTTP_SERVER_PROCESS_ID: ProcessId = ProcessId::new(Some("http_server"), "sys", "uqbar");
    pub static ref KERNEL_PROCESS_ID: ProcessId = ProcessId::new(Some("kernel"), "sys", "uqbar");
    pub static ref TERMINAL_PROCESS_ID: ProcessId = ProcessId::new(Some("terminal"), "terminal", "uqbar");
    pub static ref TIMER_PROCESS_ID: ProcessId = ProcessId::new(Some("timer"), "sys", "uqbar");
    pub static ref VFS_PROCESS_ID: ProcessId = ProcessId::new(Some("vfs"), "sys", "uqbar");
}

//
// types shared between kernel and processes. frustratingly, this is an exact copy
// of the types in process_lib/src/kernel_types.rs
// this is because even though the types are identical, they will not match when
// used in the kernel context which generates bindings differently than the process
// standard library. make sure to keep this synced with kernel_types.rs
//
pub type Context = Vec<u8>;
pub type NodeId = String; // QNS domain name

/// process ID is a formatted unique identifier that contains
/// the publishing node's ID, the package name, and finally the process name.
/// the process name can be a random number, or a name chosen by the user.
/// the formatting is as follows:
/// `[process name]:[package name]:[node ID]`
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct ProcessId {
    process_name: String,
    package_name: String,
    publisher_node: NodeId,
}

#[allow(dead_code)]
impl ProcessId {
    /// generates a random u64 number if process_name is not declared
    pub fn new(process_name: Option<&str>, package_name: &str, publisher_node: &str) -> Self {
        ProcessId {
            process_name: process_name
                .unwrap_or(&rand::random::<u64>().to_string())
                .into(),
            package_name: package_name.into(),
            publisher_node: publisher_node.into(),
        }
    }
    pub fn from_str(input: &str) -> Result<Self, ProcessIdParseError> {
        // split string on colons into 3 segments
        let mut segments = input.split(':');
        let process_name = segments
            .next()
            .ok_or(ProcessIdParseError::MissingField)?
            .to_string();
        let package_name = segments
            .next()
            .ok_or(ProcessIdParseError::MissingField)?
            .to_string();
        let publisher_node = segments
            .next()
            .ok_or(ProcessIdParseError::MissingField)?
            .to_string();
        if segments.next().is_some() {
            return Err(ProcessIdParseError::TooManyColons);
        }
        Ok(ProcessId {
            process_name,
            package_name,
            publisher_node,
        })
    }
    pub fn process(&self) -> &str {
        &self.process_name
    }
    pub fn package(&self) -> &str {
        &self.package_name
    }
    pub fn publisher(&self) -> &str {
        &self.publisher_node
    }
    pub fn en_wit(&self) -> wit::ProcessId {
        wit::ProcessId {
            process_name: self.process_name.clone(),
            package_name: self.package_name.clone(),
            publisher_node: self.publisher_node.clone(),
        }
    }
    pub fn de_wit(wit: wit::ProcessId) -> ProcessId {
        ProcessId {
            process_name: wit.process_name,
            package_name: wit.package_name,
            publisher_node: wit.publisher_node,
        }
    }
}

#[derive(Debug)]
pub enum ProcessIdParseError {
    TooManyColons,
    MissingField,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Address {
    pub node: NodeId,
    pub process: ProcessId,
}

impl Address {
    pub fn en_wit(&self) -> wit::Address {
        wit::Address {
            node: self.node.clone(),
            process: self.process.en_wit(),
        }
    }
    pub fn de_wit(wit: wit::Address) -> Address {
        Address {
            node: wit.node,
            process: ProcessId {
                process_name: wit.process.process_name,
                package_name: wit.process.package_name,
                publisher_node: wit.process.publisher_node,
            },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Payload {
    pub mime: Option<String>, // MIME type
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub inherit: bool,
    pub expects_response: Option<u64>, // number of seconds until timeout
    pub ipc: Vec<u8>,
    pub metadata: Option<String>, // JSON-string
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub inherit: bool,
    pub ipc: Vec<u8>,
    pub metadata: Option<String>, // JSON-string
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Message {
    Request(Request),
    Response((Response, Option<Context>)),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct Capability {
    pub issuer: Address,
    pub params: String, // JSON-string
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct SignedCapability {
    pub issuer: Address,
    pub params: String,     // JSON-string
    pub signature: Vec<u8>, // signed by the kernel, so we can verify that the kernel issued it
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SendError {
    pub kind: SendErrorKind,
    pub target: Address,
    pub message: Message,
    pub payload: Option<Payload>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SendErrorKind {
    Offline,
    Timeout,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OnPanic {
    None,
    Restart,
    Requests(Vec<(Address, Request, Option<Payload>)>),
}

impl OnPanic {
    pub fn is_restart(&self) -> bool {
        match self {
            OnPanic::None => false,
            OnPanic::Restart => true,
            OnPanic::Requests(_) => false,
        }
    }
}

//
// display impls
//

impl std::fmt::Display for ProcessId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.process(),
            self.package(),
            self.publisher()
        )
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}@{}", self.node, self.process)
    }
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Message::Request(request) => write!(
                f,
                "Request(\n        inherit: {},\n        expects_response: {:?},\n        ipc: {},\n        metadata: {}\n    )",
                request.inherit,
                request.expects_response,
                match serde_json::from_slice::<serde_json::Value>(&request.ipc) {
                    Ok(json) => format!("{}", json),
                    Err(_) => format!("{:?}", request.ipc),
                },
                &request.metadata.as_ref().unwrap_or(&"None".into()),
            ),
            Message::Response((response, context)) => write!(
                f,
                "Response(\n        inherit: {},\n        ipc: {},\n        metadata: {},\n        context: {}\n    )",
                response.inherit,
                match serde_json::from_slice::<serde_json::Value>(&response.ipc) {
                    Ok(json) => format!("{}", json),
                    Err(_) => format!("{:?}", response.ipc),
                },
                &response.metadata.as_ref().unwrap_or(&"None".into()),
                if context.is_none() {
                    "None".into()
                } else {
                    match serde_json::from_slice::<serde_json::Value>(context.as_ref().unwrap()) {
                        Ok(json) => format!("{}", json),
                        Err(_) => format!("{:?}", context.as_ref().unwrap()),
                    }
                },
            ),
        }
    }
}

//
// conversions between wit types and kernel types (annoying!)
//

pub fn de_wit_request(wit: wit::Request) -> Request {
    Request {
        inherit: wit.inherit,
        expects_response: wit.expects_response,
        ipc: wit.ipc,
        metadata: wit.metadata,
    }
}

pub fn en_wit_request(request: Request) -> wit::Request {
    wit::Request {
        inherit: request.inherit,
        expects_response: request.expects_response,
        ipc: request.ipc,
        metadata: request.metadata,
    }
}

pub fn de_wit_response(wit: wit::Response) -> Response {
    Response {
        inherit: wit.inherit,
        ipc: wit.ipc,
        metadata: wit.metadata,
    }
}

pub fn en_wit_response(response: Response) -> wit::Response {
    wit::Response {
        inherit: response.inherit,
        ipc: response.ipc,
        metadata: response.metadata,
    }
}

pub fn de_wit_payload(wit: Option<wit::Payload>) -> Option<Payload> {
    match wit {
        None => None,
        Some(wit) => Some(Payload {
            mime: wit.mime,
            bytes: wit.bytes,
        }),
    }
}

pub fn en_wit_payload(load: Option<Payload>) -> Option<wit::Payload> {
    match load {
        None => None,
        Some(load) => Some(wit::Payload {
            mime: load.mime,
            bytes: load.bytes,
        }),
    }
}

pub fn de_wit_signed_capability(wit: wit::SignedCapability) -> SignedCapability {
    SignedCapability {
        issuer: Address {
            node: wit.issuer.node,
            process: ProcessId {
                process_name: wit.issuer.process.process_name,
                package_name: wit.issuer.process.package_name,
                publisher_node: wit.issuer.process.publisher_node,
            },
        },
        params: wit.params,
        signature: wit.signature,
    }
}

pub fn _en_wit_signed_capability(cap: SignedCapability) -> wit::SignedCapability {
    wit::SignedCapability {
        issuer: cap.issuer.en_wit(),
        params: cap.params,
        signature: cap.signature,
    }
}

pub fn en_wit_message(message: Message) -> wit::Message {
    match message {
        Message::Request(request) => wit::Message::Request(en_wit_request(request)),
        Message::Response((response, context)) => {
            wit::Message::Response((en_wit_response(response), context))
        }
    }
}

pub fn en_wit_send_error(error: SendError) -> wit::SendError {
    wit::SendError {
        kind: en_wit_send_error_kind(error.kind),
        message: en_wit_message(error.message),
        payload: en_wit_payload(error.payload),
    }
}

pub fn en_wit_send_error_kind(kind: SendErrorKind) -> wit::SendErrorKind {
    match kind {
        SendErrorKind::Offline => wit::SendErrorKind::Offline,
        SendErrorKind::Timeout => wit::SendErrorKind::Timeout,
    }
}

pub fn de_wit_on_panic(wit: wit::OnPanic) -> OnPanic {
    match wit {
        wit::OnPanic::None => OnPanic::None,
        wit::OnPanic::Restart => OnPanic::Restart,
        wit::OnPanic::Requests(reqs) => OnPanic::Requests(
            reqs.into_iter()
                .map(|(address, request, payload)| {
                    (
                        Address::de_wit(address),
                        de_wit_request(request),
                        de_wit_payload(payload),
                    )
                })
                .collect(),
        ),
    }
}
//
// END SYNC WITH kernel_types.rs
//

//
// internal message pipes between kernel and runtime modules
//

// keeps the from address so we know where to pipe error
pub type NetworkErrorSender = tokio::sync::mpsc::Sender<WrappedSendError>;
pub type NetworkErrorReceiver = tokio::sync::mpsc::Receiver<WrappedSendError>;

pub type MessageSender = tokio::sync::mpsc::Sender<KernelMessage>;
pub type MessageReceiver = tokio::sync::mpsc::Receiver<KernelMessage>;

pub type PrintSender = tokio::sync::mpsc::Sender<Printout>;
pub type PrintReceiver = tokio::sync::mpsc::Receiver<Printout>;

pub type DebugSender = tokio::sync::mpsc::Sender<DebugCommand>;
pub type DebugReceiver = tokio::sync::mpsc::Receiver<DebugCommand>;

pub type CapMessageSender = tokio::sync::mpsc::Sender<CapMessage>;
pub type CapMessageReceiver = tokio::sync::mpsc::Receiver<CapMessage>;

//
// types used for UQI: uqbar's identity system
//

#[derive(Debug, Serialize, Deserialize)]
pub struct Registration {
    pub username: NodeId,
    pub password: String,
    pub direct: bool,
}

#[derive(Debug)]
pub struct Keyfile {
    pub username: String,
    pub routers: Vec<String>,
    pub networking_keypair: signature::Ed25519KeyPair,
    pub jwt_secret_bytes: Vec<u8>,
    pub file_key: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyfileVet {
    pub password: String,
    pub keyfile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyfileVetted {
    pub username: String,
    pub networking_key: String,
    pub routers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootInfo {
    pub password: String,
    pub keyfile: String,
    pub username: String,
    pub reset: bool,
    pub direct: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Identity {
    pub name: NodeId,
    pub networking_key: String,
    pub ws_routing: Option<(String, u16)>,
    pub allowed_routers: Vec<NodeId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityTransaction {
    pub from: String,
    pub signature: Option<String>,
    pub to: String, // contract address
    pub town_id: u32,
    pub calldata: Identity,
    pub nonce: String,
}

//
// kernel types that runtime modules use
//

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessMetadata {
    pub our: Address,
    pub wasm_bytes_handle: u128,
    pub on_panic: OnPanic,
    pub public: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KernelMessage {
    pub id: u64,
    pub source: Address,
    pub target: Address,
    pub rsvp: Rsvp,
    pub message: Message,
    pub payload: Option<Payload>,
    pub signed_capabilities: Option<Vec<SignedCapability>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WrappedSendError {
    pub id: u64,
    pub source: Address,
    pub error: SendError,
}

/// A terminal printout. Verbosity level is from low to high, and for
/// now, only 0 and 1 are used. Level 0 is always printed, level 1 is
/// only printed if the terminal is in verbose mode. Numbers greater
/// than 1 are reserved for future use and will be ignored for now.
pub struct Printout {
    pub verbosity: u8,
    pub content: String,
}

//  kernel sets in case, e.g.,
//   A requests response from B does not request response from C
//   -> kernel sets `Some(A) = Rsvp` for B's request to C
pub type Rsvp = Option<Address>;

//
//  boot/startup specific types???
//

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BootOutboundRequest {
    pub target_process: ProcessId,
    pub json: Option<String>,
    pub bytes: Option<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DebugCommand {
    Toggle,
    Step,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum KernelCommand {
    Booted,
    StartProcess {
        id: ProcessId,
        wasm_bytes_handle: u128,
        on_panic: OnPanic,
        initial_capabilities: HashSet<SignedCapability>,
        public: bool,
    },
    KillProcess(ProcessId), // this is extrajudicial killing: we might lose messages!
    // kernel only
    RebootProcess {
        process_id: ProcessId,
        persisted: PersistedProcess,
    },
    Shutdown,
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum CapMessage {
    Add {
        on: ProcessId,
        cap: Capability,
        responder: tokio::sync::oneshot::Sender<bool>,
    },
    Drop {
        // not used yet!
        on: ProcessId,
        cap: Capability,
        responder: tokio::sync::oneshot::Sender<bool>,
    },
    Has {
        // a bool is given in response here
        on: ProcessId,
        cap: Capability,
        responder: tokio::sync::oneshot::Sender<bool>,
    },
    GetAll {
        on: ProcessId,
        responder: tokio::sync::oneshot::Sender<HashSet<Capability>>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum KernelResponse {
    StartedProcess,
    StartProcessError,
    KilledProcess(ProcessId),
}

pub type ProcessMap = HashMap<ProcessId, PersistedProcess>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistedProcess {
    pub wasm_bytes_handle: u128,
    // pub drive: String,
    // pub full_path: String,
    pub on_panic: OnPanic,
    pub capabilities: HashSet<Capability>,
    pub public: bool, // marks if a process allows messages from any process
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessContext {
    // store ultimate in order to set prompting message if needed
    pub prompting_message: Option<KernelMessage>,
    // can be empty if a request doesn't set context, but still needs to inherit
    pub context: Option<Context>,
}

//
// runtime-module-specific types
//

//
// filesystem.rs types
//

pub type PackageVersion = (u32, u32, u32);

/// the type that gets deserialized from `metadata.json` in a package
#[derive(Debug, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub package: String,
    pub publisher: String,
    pub version: PackageVersion,
    pub description: Option<String>,
    pub website: Option<String>,
}

/// the type that gets deserialized from each entry in the array in `manifest.json`
#[derive(Debug, Serialize, Deserialize)]
pub struct PackageManifestEntry {
    pub process_name: String,
    pub process_wasm_path: String,
    pub on_panic: OnPanic,
    pub request_networking: bool,
    pub request_messaging: Vec<String>,
    pub public: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum FsAction {
    Write(Option<u128>),
    WriteOffset((u128, u64)),
    Append(Option<u128>),
    Read(u128),
    ReadChunk(ReadChunkRequest),
    Delete(u128),
    Length(u128),
    SetLength((u128, u64)),
    GetState(ProcessId),
    SetState(ProcessId),
    DeleteState(ProcessId),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ReadChunkRequest {
    pub file: u128,
    pub start: u64,
    pub length: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum FsResponse {
    Write(u128),
    Read(u128),
    ReadChunk(u128), //  TODO: remove?
    Append(u128),
    Delete(u128),
    Length(u64),
    GetState,
    SetState,
}

#[derive(Debug)]
pub struct S3Config {
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
    pub bucket: String,
    pub endpoint: String,
}

#[derive(Debug)]
pub struct FsConfig {
    pub s3_config: Option<S3Config>,
    pub mem_buffer_limit: usize,
    pub read_cache_limit: usize,
    pub chunk_size: usize,
    pub flush_to_cold_interval: usize,
    pub encryption: bool,
    pub cloud_enabled: bool,
    // pub flush_to_wal_interval: usize,
}

#[derive(Error, Debug, Serialize, Deserialize)]
pub enum FsError {
    #[error("fs: Bytes payload required for {action}.")]
    BadBytes { action: String },
    #[error(
        "fs: JSON payload could not be parsed to FsAction: {:?}, error: {:?}.",
        json,
        error
    )]
    BadJson { json: String, error: String },
    #[error("fs: No JSON payload.")]
    NoJson,
    #[error("fs: Read failed to file {file}: {error}.")]
    ReadFailed { file: u128, error: String },
    #[error("fs: Write failed to file {file}: {error}.")]
    WriteFailed { file: u128, error: String },
    #[error("fs: file not found: {file}")]
    NotFound { file: u128 },
    #[error("fs: S3 error: {error}")]
    S3Error { error: String },
    #[error("fs: IO error: {error}")]
    IOError { error: String },
    #[error("fs: Encryption error: {error}")]
    EncryptionError { error: String },
    #[error("fs: Limit error: {error}")]
    LimitError { error: String },
    #[error("fs: memory buffer error: {error}")]
    MemoryBufferError { error: String },
    #[error("fs: length operation error: {error}")]
    LengthError { error: String },
    #[error("fs: creating fs dir failed at path: {path}: {error}")]
    CreateInitialDirError { path: String, error: String },
}

#[allow(dead_code)]
impl FsError {
    pub fn kind(&self) -> &str {
        match *self {
            FsError::BadBytes { .. } => "BadBytes",
            FsError::BadJson { .. } => "BadJson",
            FsError::NoJson { .. } => "NoJson",
            FsError::ReadFailed { .. } => "ReadFailed",
            FsError::WriteFailed { .. } => "WriteFailed",
            FsError::S3Error { .. } => "S3Error",
            FsError::IOError { .. } => "IOError",
            FsError::EncryptionError { .. } => "EncryptionError",
            FsError::LimitError { .. } => "LimitError",
            FsError::MemoryBufferError { .. } => "MemoryBufferError",
            FsError::NotFound { .. } => "NotFound",
            FsError::LengthError { .. } => "LengthError",
            FsError::CreateInitialDirError { .. } => "CreateInitialDirError",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VfsRequest {
    pub drive: String,
    pub action: VfsAction,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum VfsAction {
    New,
    Add {
        full_path: String,
        entry_type: AddEntryType,
    },
    Rename {
        full_path: String,
        new_full_path: String,
    },
    Delete(String),
    WriteOffset {
        full_path: String,
        offset: u64,
    },
    Append(String),
    SetSize {
        full_path: String,
        size: u64,
    },
    GetPath(u128),
    GetHash(String),
    GetEntry(String),
    GetFileChunk {
        full_path: String,
        offset: u64,
        length: u64,
    },
    GetEntryLength(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AddEntryType {
    Dir,
    NewFile,                     //  add a new file to fs and add name in vfs
    ExistingFile { hash: u128 }, //  link an existing file in fs to a new name in vfs
    ZipArchive,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum GetEntryType {
    Dir,
    File,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum VfsResponse {
    Ok,
    Err(VfsError),
    GetPath(Option<String>),
    GetHash(Option<u128>),
    GetEntry {
        // file bytes in payload, if entry was a file
        is_file: bool,
        children: Vec<String>,
    },
    GetFileChunk, // chunk in payload, if file exists
    GetEntryLength(Option<u64>),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum VfsError {
    BadJson,
    BadPayload,
    BadDriveName,
    BadDescriptor,
    NoCap,
    EntryNotFound,
    PersistError,
    InternalError, // String
}

#[allow(dead_code)]
impl VfsError {
    pub fn kind(&self) -> &str {
        match *self {
            VfsError::BadJson => "BadJson",
            VfsError::BadPayload => "BadPayload",
            VfsError::BadDriveName => "BadDriveName",
            VfsError::BadDescriptor => "BadDescriptor",
            VfsError::NoCap => "NoCap",
            VfsError::EntryNotFound => "EntryNotFound",
            VfsError::PersistError => "PersistError",
            VfsError::InternalError => "InternalError",
        }
    }
}

//
// http_client.rs types
//

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpClientRequest {
    pub uri: String,
    pub method: String,
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpClientResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
}

#[derive(Error, Debug, Serialize, Deserialize)]
pub enum HttpClientError {
    #[error("http_client: rsvp is None but message is expecting response")]
    BadRsvp,
    #[error("http_client: no json in request")]
    NoJson,
    #[error(
        "http_client: JSON payload could not be parsed to HttpClientRequest: {error}. Got {:?}.",
        json
    )]
    BadJson { json: String, error: String },
    #[error("http_client: http method not supported: {:?}", method)]
    BadMethod { method: String },
    #[error("http_client: failed to execute request {:?}", error)]
    RequestFailed { error: String },
}

#[allow(dead_code)]
impl HttpClientError {
    pub fn kind(&self) -> &str {
        match *self {
            HttpClientError::BadRsvp { .. } => "BadRsvp",
            HttpClientError::NoJson { .. } => "NoJson",
            HttpClientError::BadJson { .. } => "BadJson",
            HttpClientError::BadMethod { .. } => "BadMethod",
            HttpClientError::RequestFailed { .. } => "RequestFailed",
        }
    }
}

//
// custom kernel displays
//

impl std::fmt::Display for KernelMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{{\n    id: {},\n    source: {},\n    target: {},\n    rsvp: {},\n    message: {},\n    payload: {}\n}}",
            self.id,
            self.source,
            self.target,
            match &self.rsvp {
                Some(rsvp) => rsvp.to_string(),
                None => "None".to_string()
            },
            self.message,
            self.payload.is_some(),
        )
    }
}

//
// http_server.rs types
//

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Option<Vec<u8>>, // TODO does this use a lot of memory?
}

#[derive(Error, Debug, Serialize, Deserialize)]
pub enum HttpServerError {
    #[error("http_server: json is None")]
    NoJson,
    #[error("http_server: response not ok")]
    ResponseError,
    #[error("http_server: bytes are None")]
    NoBytes,
    #[error(
        "http_server: JSON payload could not be parsed to HttpClientRequest: {error}. Got {:?}.",
        json
    )]
    BadJson { json: String, error: String },
    #[error("http_server: path binding error:  {:?}", error)]
    PathBind { error: String },
}

#[allow(dead_code)]
impl HttpServerError {
    pub fn kind(&self) -> &str {
        match *self {
            HttpServerError::NoJson { .. } => "NoJson",
            HttpServerError::NoBytes { .. } => "NoBytes",
            HttpServerError::BadJson { .. } => "BadJson",
            HttpServerError::ResponseError { .. } => "ResponseError",
            HttpServerError::PathBind { .. } => "PathBind",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    pub username: String,
    pub expiration: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WebSocketServerTarget {
    pub node: String,
    pub id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WebSocketPush {
    pub target: WebSocketServerTarget,
    pub is_text: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerAction {
    pub action: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum HttpServerMessage {
    BindPath {
        path: String,
        authenticated: bool,
        local_only: bool,
    },
    WebSocketPush(WebSocketPush),
    ServerAction(ServerAction),
    WsRegister(WsRegister),                 // Coming from a proxy
    WsProxyDisconnect(WsProxyDisconnect),   // Coming from a proxy
    WsMessage(WsMessage),                   // Coming from a proxy
    EncryptedWsMessage(EncryptedWsMessage), // Coming from a proxy
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WsRegister {
    pub ws_auth_token: String,
    pub auth_token: String,
    pub channel_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WsProxyDisconnect {
    // Doesn't require auth because it's coming from the proxy
    pub channel_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WsMessage {
    pub ws_auth_token: String,
    pub auth_token: String,
    pub channel_id: String,
    pub target: Address,
    pub json: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedWsMessage {
    pub ws_auth_token: String,
    pub auth_token: String,
    pub channel_id: String,
    pub target: Address,
    pub encrypted: String, // Encrypted JSON as hex with the 32-byte authentication tag appended
    pub nonce: String,     // Hex of the 12-byte nonce
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WebSocketClientMessage {
    WsRegister(WsRegister),
    WsMessage(WsMessage),
    EncryptedWsMessage(EncryptedWsMessage),
}
// http_server End

// encryptor Start
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetKeyAction {
    pub channel_id: String,
    pub public_key_hex: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecryptAndForwardAction {
    pub channel_id: String,
    pub forward_to: Address, // node, process
    pub json: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptAndForwardAction {
    pub channel_id: String,
    pub forward_to: Address, // node, process
    pub json: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecryptAction {
    pub channel_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptAction {
    pub channel_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum EncryptorMessage {
    GetKey(GetKeyAction),
    DecryptAndForward(DecryptAndForwardAction),
    EncryptAndForward(EncryptAndForwardAction),
    Decrypt(DecryptAction),
    Encrypt(EncryptAction),
}
// encryptor End
