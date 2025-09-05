use hyperprocess_macro::hyperprocess;
use hyperware_process_lib::hyperapp::{add_response_header, get_path, send, SaveOptions};
use hyperware_process_lib::logging::{debug, error, info, init_logging, Level};
use hyperware_process_lib::our;
use hyperware_process_lib::vfs::{
    self, create_drive, vfs_request, FileType, VfsAction, VfsResponse,
};
use std::collections::HashMap;

const ICON: &str = include_str!("./icon");
const PROCESS_ID_LINK: &str = "explorer:file-explorer:sys";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub created: u64,
    pub modified: u64,
    pub is_directory: bool,
    pub permissions: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum AuthScheme {
    Public,
    Private,
}

#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
struct FileExplorerState {
    // HashMap to track shared files and their auth schemes
    shared_files: HashMap<String, AuthScheme>,
    // Current working directory for the user
    cwd: String,
}

#[hyperprocess(
    name = "file-explorer",
    ui = Some(HttpBindingConfig::default().secure_subdomain(true)),
    endpoints = vec![
        Binding::Http {
            path: "/api",
            config: HttpBindingConfig::default().secure_subdomain(true),
        },
        Binding::Ws {
            path: "/ws",
            config: WsBindingConfig::default().secure_subdomain(true),
        },
        Binding::Http {
            path: "/shared/*",
            config: HttpBindingConfig::default().authenticated(false),
        }
    ],
    save_config = SaveOptions::Never,
    wit_world = "file-explorer-sys-v0",
)]
impl FileExplorerState {
    #[init]
    async fn init(&mut self) {
        init_logging(Level::DEBUG, Level::INFO, None, None, None).unwrap();

        // Create home drive for the user
        let package_id = our().package_id();
        match create_drive(package_id.clone(), "home", Some(5)) {
            Ok(home_path) => {
                info!("Created home drive at: {}", home_path);
                self.cwd = home_path;
            }
            Err(e) => {
                error!(
                    "Failed to create home drive: {:?}. Using root as default.",
                    e
                );
                self.cwd = "/".to_string();
            }
        }

        hyperware_process_lib::homepage::add_to_homepage(
            "File Explorer",
            Some(ICON),
            Some(""),
            None,
        );
    }

    #[http]
    async fn list_directory(&mut self, path: String) -> Result<Vec<FileInfo>, String> {
        info!("list_directory called with path: {}", path);

        // For root path, read from VFS root to get all drives
        let vfs_path = if path == "/" || path.is_empty() {
            "/".to_string()
        } else {
            path.clone()
        };

        // Just list the current directory - no recursion
        list_directory_contents(&vfs_path).await
    }

    #[http]
    async fn create_file(&mut self, path: String, content: Vec<u8>) -> Result<FileInfo, String> {
        info!("create_file called with path: {}", path);

        let vfs_path = path.clone();
        debug!("VFS path: {}", vfs_path);

        // Create file and write content
        let file = vfs::create_file(&vfs_path, Some(5))
            .map_err(|e| format!("Failed to create file: {}", e))?;

        file.write(&content)
            .map_err(|e| format!("Failed to write file: {}", e))?;

        // Get metadata for response
        let meta = file
            .metadata()
            .map_err(|e| format!("Failed to get metadata: {}", e))?;

        Ok(FileInfo {
            name: path.split('/').last().unwrap_or("").to_string(),
            path,
            size: meta.len,
            created: 0,
            modified: 0,
            is_directory: false,
            permissions: "rw".to_string(),
        })
    }

    #[http]
    async fn read_file(&mut self, path: String) -> Result<Vec<u8>, String> {
        info!("read_file called with path: {}", path);

        let vfs_path = path.clone();

        let file = vfs::open_file(&vfs_path, false, Some(5))
            .map_err(|e| format!("Failed to open file: {}", e))?;

        file.read()
            .map_err(|e| format!("Failed to read file: {}", e))
    }

    #[http]
    async fn update_file(&mut self, path: String, content: Vec<u8>) -> Result<FileInfo, String> {
        info!("update_file called with path: {}", path);

        let vfs_path = path.clone();

        let file = vfs::open_file(&vfs_path, false, Some(5))
            .map_err(|e| format!("Failed to open file: {}", e))?;

        file.write(&content)
            .map_err(|e| format!("Failed to write file: {}", e))?;

        let meta = file
            .metadata()
            .map_err(|e| format!("Failed to get metadata: {}", e))?;

        Ok(FileInfo {
            name: path.split('/').last().unwrap_or("").to_string(),
            path,
            size: meta.len,
            created: 0,
            modified: 0,
            is_directory: false,
            permissions: "rw".to_string(),
        })
    }

    #[http]
    async fn delete_file(&mut self, path: String) -> Result<bool, String> {
        info!("delete_file called with path: {}", path);

        let vfs_path = path.clone();

        vfs::remove_file(&vfs_path, Some(5))
            .await
            .map_err(|e| format!("Failed to delete file: {}", e))?;

        Ok(true)
    }

    #[http]
    async fn create_directory(&mut self, path: String) -> Result<FileInfo, String> {
        info!("create_directory called with path: {}", path);

        let vfs_path = path.clone();

        let _dir = vfs::open_dir(&vfs_path, true, Some(5))
            .map_err(|e| format!("Failed to create directory: {}", e))?;

        Ok(FileInfo {
            name: path.split('/').last().unwrap_or("").to_string(),
            path,
            size: 0,
            created: 0,
            modified: 0,
            is_directory: true,
            permissions: "rw".to_string(),
        })
    }

    #[http]
    async fn delete_directory(&mut self, path: String) -> Result<bool, String> {
        info!("delete_directory called with path: {}", path);

        let vfs_path = path.clone();
        let timeout = 5;

        // Create a VFS request with RemoveDirAll action to handle non-empty directories
        let request = vfs_request(&vfs_path, VfsAction::RemoveDirAll).expects_response(timeout);

        // Send the request and await response
        let response: Result<VfsResponse, _> = send(request).await;

        match response {
            Ok(VfsResponse::Ok) => Ok(true),
            Ok(VfsResponse::Err(e)) => Err(format!("Failed to delete directory: {:?}", e)),
            Ok(_) => Err("Unexpected response from VFS".to_string()),
            Err(e) => Err(format!("Failed to send VFS request: {}", e)),
        }
    }

    #[http]
    async fn upload_file(
        &mut self,
        path: String,
        filename: String,
        content: Vec<u8>,
    ) -> Result<FileInfo, String> {
        let full_path = format!("{}/{}", path, filename);
        self.create_file(full_path, content).await
    }

    #[http]
    async fn share_file(&mut self, path: String, auth: AuthScheme) -> Result<String, String> {
        // Generate share ID from path hash
        let share_id = format!("{:x}", md5::compute(&path));

        // Add to shared_files HashMap
        self.shared_files.insert(path.clone(), auth);

        // Return share link with full path
        Ok(format!("/{PROCESS_ID_LINK}/shared/{share_id}"))
    }

    #[http]
    async fn unshare_file(&mut self, path: String) -> Result<bool, String> {
        Ok(self.shared_files.remove(&path).is_some())
    }

    #[http]
    async fn get_share_link(&mut self, path: String) -> Result<Option<String>, String> {
        // Check if file is shared
        if self.shared_files.contains_key(&path) {
            let share_id = format!("{:x}", md5::compute(&path));
            Ok(Some(format!("/{PROCESS_ID_LINK}/shared/{share_id}")))
        } else {
            Ok(None)
        }
    }

    #[http]
    async fn serve_shared_file(&mut self) -> Result<Vec<u8>, String> {
        // Use get_path() to handle routing
        let request_path = get_path();

        // Extract the file path from the request
        if let Some(request_path_str) = request_path {
            if let Some(share_id) = request_path_str.strip_prefix("/shared/") {
                // Find the original path from share_id
                for (path, auth_scheme) in &self.shared_files {
                    if format!("{:x}", md5::compute(path)) == share_id {
                        match auth_scheme {
                            AuthScheme::Public => {
                                // Extract filename from path
                                let filename = path.split('/').last().unwrap_or("download");

                                // Set Content-Disposition header to preserve original filename
                                add_response_header(
                                    "Content-Disposition".to_string(),
                                    format!("attachment; filename=\"{}\"", filename),
                                );

                                // Set appropriate Content-Type based on file extension
                                let content_type = match filename.split('.').last() {
                                    Some("txt") => "text/plain",
                                    Some("html") | Some("htm") => "text/html",
                                    Some("css") => "text/css",
                                    Some("js") => "application/javascript",
                                    Some("json") => "application/json",
                                    Some("png") => "image/png",
                                    Some("jpg") | Some("jpeg") => "image/jpeg",
                                    Some("gif") => "image/gif",
                                    Some("pdf") => "application/pdf",
                                    Some("zip") => "application/zip",
                                    _ => "application/octet-stream",
                                };
                                add_response_header(
                                    "Content-Type".to_string(),
                                    content_type.to_string(),
                                );

                                // Read and return file content
                                return self.read_file(path.clone()).await;
                            }
                            AuthScheme::Private => {
                                return Err("Access denied: Private file".to_string());
                            }
                        }
                    }
                }
                Err("File not found or not shared".to_string())
            } else {
                Err("Invalid shared file path".to_string())
            }
        } else {
            Err("No request path provided".to_string())
        }
    }

    #[http]
    async fn get_current_directory(&mut self) -> Result<String, String> {
        info!("get_current_directory called, returning: {}", self.cwd);
        Ok(self.cwd.clone())
    }

    #[http]
    async fn set_current_directory(&mut self, path: String) -> Result<String, String> {
        self.cwd = path.clone();
        Ok(path)
    }

    #[http]
    async fn move_file(&mut self, source: String, destination: String) -> Result<FileInfo, String> {
        // Read file content
        let content = self.read_file(source.clone()).await?;

        // Create file at destination
        let file_info = self.create_file(destination, content).await?;

        // Delete source file
        self.delete_file(source).await?;

        Ok(file_info)
    }

    #[http]
    async fn copy_file(&mut self, source: String, destination: String) -> Result<FileInfo, String> {
        // Read file content
        let content = self.read_file(source).await?;

        // Create file at destination
        self.create_file(destination, content).await
    }
}

// Helper function to list directory contents with 2 levels of depth
async fn list_directory_contents(path: &str) -> Result<Vec<FileInfo>, String> {
    debug!("list_directory_contents: path='{}'", path);

    // Open directory
    let dir = vfs::Directory {
        path: path.to_string(),
        timeout: 5,
    };

    // Read directory entries
    let entries = dir
        .read()
        .map_err(|e| format!("Failed to read directory '{}': {}", path, e))?;

    debug!("VFS returned {} entries for path '{}'", entries.len(), path);

    let mut all_files = Vec::new();

    // Convert to FileInfo - Level 1
    for (i, entry) in entries.iter().enumerate() {
        debug!(
            "Entry[{}]: path='{}', file_type={:?}",
            i, entry.path, entry.file_type
        );

        // VFS already provides absolute paths in entry.path
        let full_path = entry.path.clone();

        // Extract filename from the path
        let filename = entry.path.split('/').last().unwrap_or("").to_string();

        debug!(
            "Constructed: filename='{}', full_path='{}'",
            filename, full_path
        );

        if entry.file_type == FileType::Directory {
            // Get directory size
            let sub_dir = vfs::Directory {
                path: full_path.clone(),
                timeout: 5,
            };

            let dir_size = match sub_dir.read() {
                Ok(contents) => {
                    let count = contents.len() as u64;
                    debug!("Directory '{}' has {} items", full_path, count);
                    count
                }
                Err(e) => {
                    error!("Failed to read subdirectory '{}': {}", full_path, e);
                    0
                }
            };

            let file_info = FileInfo {
                name: filename,
                path: full_path.clone(),
                size: dir_size,
                created: 0,
                modified: 0,
                is_directory: true,
                permissions: "rw".to_string(),
            };

            all_files.push(file_info);

            // Load one level deep into directories
            let sub_dir2 = vfs::Directory {
                path: full_path.clone(),
                timeout: 5,
            };

            if let Ok(sub_entries) = sub_dir2.read() {
                debug!(
                    "Loading {} sub-entries from '{}'",
                    sub_entries.len(),
                    full_path
                );

                for sub_entry in sub_entries {
                    // VFS already provides absolute paths in sub_entry.path
                    let sub_full_path = sub_entry.path.clone();
                    let sub_filename = sub_entry.path.split('/').last().unwrap_or("").to_string();

                    debug!(
                        "Sub-entry: path='{}', filename='{}', file_type={:?}",
                        sub_full_path, sub_filename, sub_entry.file_type
                    );

                    if sub_entry.file_type == FileType::Directory {
                        all_files.push(FileInfo {
                            name: sub_filename,
                            path: sub_full_path,
                            size: 0, // Don't load deeper
                            created: 0,
                            modified: 0,
                            is_directory: true,
                            permissions: "rw".to_string(),
                        });
                    } else {
                        // For files, try to get metadata
                        if let Ok(meta) = vfs::metadata(&sub_full_path, Some(5)).await {
                            all_files.push(FileInfo {
                                name: sub_filename,
                                path: sub_full_path,
                                size: meta.len,
                                created: 0,
                                modified: 0,
                                is_directory: false,
                                permissions: "rw".to_string(),
                            });
                        }
                    }
                }
            }
        } else {
            // For files, get metadata
            let meta = vfs::metadata(&full_path, Some(5))
                .await
                .map_err(|e| format!("Failed to get metadata for '{}': {}", entry.path, e))?;

            all_files.push(FileInfo {
                name: filename,
                path: full_path,
                size: meta.len,
                created: 0,
                modified: 0,
                is_directory: false,
                permissions: "rw".to_string(),
            });
        }
    }

    debug!("Returning {} files total", all_files.len());
    Ok(all_files)
}
