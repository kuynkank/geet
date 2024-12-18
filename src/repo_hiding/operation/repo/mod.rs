use serde::{Deserialize, Serialize};
use crate::file_hiding::file_log::{deserialize_metadata,copy_dir};
use crate::file_hiding::ref_log::store_ref;
use crate::repo_hiding::data_type::{CommitMetadata, RepositoryConfig};
use crate::repo_hiding::data_type::{Hash, RefType};
use crate::repo_hiding::operation::branch::{create_head, create_ref, get_head, get_ref, update_head, update_ref};
use crate::repo_hiding::operation::revision::create_revision;
use std::io;
use std::fmt;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::collections::HashSet;


#[derive(Debug, Serialize, Deserialize)] 
pub enum RepoError {
    InitializationFailed(String),
    SerializationError(String),
}

impl fmt::Display for RepoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RepoError::InitializationFailed(msg) => write!(f, "Initialization Failed: {}", msg),
            RepoError::SerializationError(msg) => write!(f, "Serialization Error: {}", msg),
        }
    }
}

impl RepositoryConfig {
    // Initializes a new repository configuration and prints it to the command aline
    pub fn init_repo(
        name: String,
        path: String,
        default_branch: String,
    ) -> Result<Self, RepoError> {
        // Check if .geet directory already exists
        let geet_path = format!("{}/.geet", path);
        if Path::new(&geet_path).exists() {
            return Err(RepoError::InitializationFailed(
                "Repository already exists in this location".to_string(),
            ));
        }
        
        // Ensure the directory exists or create it
        if !Path::new(&path).exists() {
            fs::create_dir_all(&path).map_err(|e| {
                RepoError::InitializationFailed(format!("Failed to create directory: {}", e))
            })?;
        }

        // create .geet directory
        let refs_path = format!("{}/.geet/refs", path);
        fs::create_dir_all(&refs_path).unwrap();
        let objects_path = format!("{}/.geet/objects", path);
        fs::create_dir_all(&objects_path).unwrap();
        // create the index file
        File::create(format!("{}/.geet/index", path)).unwrap();

      // Step 1: Create an initial commit
      let metadata = CommitMetadata {
        author: "System".to_string(), // Default system author for the initial commit
        message: "Initial commit".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    let initial_commit_hash = create_revision(metadata); // Use the provided `create_revision` function
    println!("Initial commit created with hash: {}", initial_commit_hash);

    // Step 2: Create the default branch reference and point it to the initial commit
    create_ref(
        RefType::Branch,
        default_branch.clone(),
        Some(initial_commit_hash.clone()),
    );

    // Step 3: Create the HEAD reference and point it to the initial commit
    create_head();
    update_head(&initial_commit_hash);


        // Create the RepositoryConfig instance
        let config = RepositoryConfig {
            name: name.clone(),
            default_branch: default_branch.clone(),
        };

        // Serialize the config to JSON format
        let serialized_config = config.serialize();

        // Print the serialized JSON to the command line
        println!("Repository configuration initialized:");
        println!("{}", serialized_config);

        Ok(config)
    }

    pub fn validate_remote_repo(path: &str) -> std::io::Result<()> {
        let geet_path = format!("{}/.geet", path);
        if !Path::new(&geet_path).exists() {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Invalid remote repository: .geet directory not found",
            ))
        } else {
            Ok(())
        }
    }

    fn copy_refs(remote_path: &str, local_path: &str)  -> std::io::Result<()> {
        let remote_refs_path = format!("{}/.geet/refs", remote_path);
        let local_refs_path = format!("{}/.geet/refs", local_path);
        copy_dir(&remote_refs_path, &local_refs_path)
    }

    pub fn clone_repo(
        remote_path: String,
        local_path: String,
    ) -> Result<(), RepoError> {
        // Validate the remote repository
        Self::validate_remote_repo(&remote_path).map_err(|e| {
            RepoError::InitializationFailed(format!("Remote repository validation failed: {}", e))
        })?;
    
        // Ensure the local path does not already exist
        if Path::new(&local_path).exists() {
            return Err(RepoError::InitializationFailed(
                "The destination path already exists.".to_string(),
            ));
        }
    
        // Create the local repository structure
        fs::create_dir_all(&local_path).map_err(|e| {
            RepoError::InitializationFailed(format!("Failed to create directory: {}", e))
        })?;
    
        // Copy the .geet directory
        let remote_geet_path = format!("{}/.geet", remote_path);
        let local_geet_path = format!("{}/.geet", local_path);
        copy_dir(&remote_geet_path, &local_geet_path).map_err(|e| {
            RepoError::InitializationFailed(format!("Failed to copy .geet directory: {}", e))
        })?;
    
        // Copy references
        Self::copy_refs(&remote_path, &local_path).map_err(|e| {
            RepoError::InitializationFailed(format!("Failed to copy refs: {}", e))
        })?;
    
        // Fetch the remote HEAD hash
    let remote_head_ref = get_ref(&"HEAD".to_string());
    let remote_head_hash = remote_head_ref
        .commit_hash
        .clone()
        .ok_or_else(|| RepoError::InitializationFailed("Remote HEAD reference is missing.".to_string()))?;

    // Update local HEAD to point to the remote HEAD hash
    update_head(&remote_head_hash);
    
        println!("Repository successfully cloned to {}", local_path);
        Ok(())
    }  

    /// Get the list of object hashes from a directory
pub fn get_object_hashes(dir_path: &str) -> io::Result<HashSet<String>> {
    let mut hashes = HashSet::new();
    if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                hashes.insert(entry.file_name().to_string_lossy().to_string());
            }
        }
    }
    Ok(hashes)
}

/// Copy only new or updated files from one directory to another
pub fn copy_new_or_updated_files(src: &str, dest: &str) -> io::Result<()> {
    let src_hashes = Self::get_object_hashes(src)?;
    let dest_hashes =Self:: get_object_hashes(dest)?;

    for hash in src_hashes.difference(&dest_hashes) {
        let src_file = format!("{}/{}", src, hash);
        let dest_file = format!("{}/{}", dest, hash);

        fs::copy(&src_file, &dest_file)?;
        println!("Copied new object: {}", hash);
    }

    Ok(())
}

    /// Pull changes from the remote repository to the local repository
    pub fn pull_repo(remote_path: String, local_path: String) -> Result<(), RepoError> {
        // Validate the remote repository
        Self::validate_remote_repo(&remote_path).map_err(|e| {
            RepoError::InitializationFailed(format!("Remote repository validation failed: {}", e))
        })?;
    
        // Fetch the current HEAD hash
        let local_head_hash = get_head(); // This should return Option<Hash>
        let remote_head_ref = get_ref(&"HEAD".to_string()); // Adjust get_ref to return Ref or Option<Ref>
        let remote_head_hash = remote_head_ref
            .commit_hash
            .clone()
            .ok_or_else(|| RepoError::InitializationFailed("Remote HEAD is missing.".to_string()))?;
    
        // Skip pull if the hashes are the same
        if local_head_hash == Some(remote_head_hash.clone()) {
            println!("No new changes to pull.");
            return Ok(());
        }
    
        // Synchronize objects
        let remote_objects_path = format!("{}/.geet/objects", remote_path);
        let local_objects_path = format!("{}/.geet/objects", local_path);
        Self::copy_new_or_updated_files(&remote_objects_path, &local_objects_path).map_err(|e| {
            RepoError::InitializationFailed(format!("Failed to pull objects: {}", e))
        })?;
    
        // Synchronize references
        let remote_refs_path = format!("{}/.geet/refs", remote_path);
        let local_refs_path = format!("{}/.geet/refs", local_path);
        Self::copy_new_or_updated_files(&remote_refs_path, &local_refs_path).map_err(|e| {
            RepoError::InitializationFailed(format!("Failed to pull refs: {}", e))
        })?;
    
        // Update HEAD
        update_head(&remote_head_hash);
    
        println!("Repository successfully pulled from {}", remote_path);
        Ok(())
    }
    
    
    


    /// Push changes from the local repository to the remote repository
    pub fn push_repo(local_path: String, remote_path: String) -> Result<(), RepoError> {
        // Validate the remote repository
        Self::validate_remote_repo(&remote_path).map_err(|e| {
            RepoError::InitializationFailed(format!("Remote repository validation failed: {}", e))
        })?;

        // Push only new or updated objects
        let local_objects_path = format!("{}/.geet/objects", &local_path);
        let remote_objects_path = format!("{}/.geet/objects", &remote_path);
        Self::copy_new_or_updated_files(&local_objects_path, &remote_objects_path).map_err(|e| {
            RepoError::InitializationFailed(format!("Failed to push new objects: {}", e))
        })?;

        // Push only updated references
        let local_refs_path = format!("{}/.geet/refs", &local_path);
        let remote_refs_path = format!("{}/.geet/refs", &remote_path);
        Self::copy_new_or_updated_files(&local_refs_path, &remote_refs_path).map_err(|e| {
            RepoError::InitializationFailed(format!("Failed to push refs: {}", e))
        })?;

        /// Update remote HEAD
    if let Some(local_head_hash) = get_head() {
        update_ref(&"HEAD".to_string(), local_head_hash);
        println!("Remote HEAD updated successfully.");
    } else {
        return Err(RepoError::InitializationFailed(
            "Local HEAD is missing.".to_string(),
        ));
    }

        println!("Repository successfully pushed to {}", remote_path);
        Ok(())
    }
}


/*Conflict Handling:
This implementation assumes no conflicts between the local and remote repositories. Conflict handling (e.g., merges) can be added based on your project requirements.
Synchronization Efficiency:
This implementation uses full directory copying for objects and references. For a more efficient solution, you could implement delta synchronization (only transfer new or updated files).
Error Handling:
All potential errors (e.g., I/O errors during copying) are converted to RepoError with descriptive messages.*/


/*How Checksum Comparison Works
Generate Checksum for Files:
Use a cryptographic hash function (e.g., SHA-256) to calculate a unique hash for each file based on its content.
Store or Compare Checksums:
Compare the calculated checksums of files in the source and destination repositories.
If the checksums differ, the file has been updated or is new.
Synchronize Files:
Copy files that have new or updated checksums to the destination repository.*/

fn main() {
    // Initialize a new repository
    match RepositoryConfig::init_repo(
        "TestRepo".to_string(),
        "test_repo".to_string(),
        "main".to_string(),
    ) {
        Ok(_) => println!("Repository initialized successfully."),
        Err(e) => eprintln!("Error initializing repository: {}", e),
    }

    // Clean up existing destination path before cloning
    if Path::new("test_repo_clone").exists() {
        std::fs::remove_dir_all("test_repo_clone")
            .expect("Failed to clean up existing destination path");
    }

    // Clone the repository
    match RepositoryConfig::clone_repo(
        "test_repo".to_string(),
        "test_repo_clone".to_string(),
    ) {
        Ok(_) => println!("Repository cloned successfully."),
        Err(e) => eprintln!("Error cloning repository: {}", e),
    }

    // Push changes to the cloned repository
    match RepositoryConfig::push_repo(
        "test_repo".to_string(),
        "test_repo_clone".to_string(),
    ) {
        Ok(_) => println!("Changes pushed successfully."),
        Err(e) => eprintln!("Error pushing changes: {}", e),
    }

    // Pull changes from the cloned repository back to the original
    match RepositoryConfig::pull_repo(
        "test_repo_clone".to_string(),
        "test_repo".to_string(),
    ) {
        Ok(_) => println!("Changes pulled successfully."),
        Err(e) => eprintln!("Error pulling changes: {}", e),
    }
}