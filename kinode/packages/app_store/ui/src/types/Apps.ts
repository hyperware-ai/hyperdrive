export interface PackageId {
    package_name: string;
    publisher_node: string;
}

export interface AppListing {
    package_id: PackageId
    tba: string
    metadata_uri: string
    metadata_hash: string
    metadata?: OnchainPackageMetadata
    auto_update: boolean
}

export type DownloadItem = {
    Dir?: DirItem;
    File?: FileItem;
};

export interface DirItem {
    name: string;
    mirroring: boolean;
}

export interface FileItem {
    name: string;
    size: number;
    manifest: string;
}

export interface MirrorCheckFile {
    node: string;
    is_online: boolean;
    error: string | null;
}

export interface Erc721Properties {
    package_name: string;
    publisher: string;
    current_version: string;
    mirrors: string[];
    code_hashes: [string, string][];
    license?: string;
    screenshots?: string[];
    wit_version?: [number, number, number];
}

export interface OnchainPackageMetadata {
    name?: string;
    description?: string;
    image?: string;
    external_url?: string;
    animation_url?: string;
    properties: Erc721Properties;
}

export interface PackageState {
    package_id: PackageId;
    our_version_hash: string;
    verified: boolean;
    caps_approved: boolean;
}

export interface PackageManifest {
    process_name: string
    process_wasm_path: string
    on_exit: string
    request_networking: boolean
    request_capabilities: any[]
    grant_capabilities: any[]
    public: boolean
}

export interface HomepageApp {
    id: string;
    process: string;
    package: string;
    publisher: string;
    path?: string;
    label: string;
    base64_icon?: string;
    widget?: string;
    order: number;
    favorite: boolean;
}
