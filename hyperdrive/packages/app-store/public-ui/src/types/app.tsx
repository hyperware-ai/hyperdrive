export interface PackageId {
    package_name: string;
    publisher_node: string;
}

export interface AppMetadata {
    name: string;
    description: string;
    image?: string;
    properties: {
        package_name: string;
        publisher: string;
        current_version: string;
        mirrors: string[];
        code_hashes: [string, string][];
        screenshots?: string[];
        license?: string;
    };
}


export interface AppListing {
    package_id: PackageId;
    tba: string;
    metadata_uri: string;
    metadata_hash: string;
    auto_update: boolean;
    metadata: AppMetadata;
}