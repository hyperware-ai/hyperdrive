use hyperware_process_lib::Address;

// Generates a timestamp string.
pub fn get_current_timestamp_str() -> String {
    let datetime = chrono::Utc::now();
    datetime.format("%Y%m%dT%H%M%SZ").to_string()
}

pub fn is_local_request(our: &Address, source: &Address) -> bool {
    our.node == source.node
}
