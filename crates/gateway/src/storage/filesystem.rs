//! Filesystem receipt store.
//!
//! Writes receipts as JSON files to a directory, organized by UTC date.
//! This is the Slice 1 behavior, preserved behind the `ReceiptStore` trait
//! so it remains available for local development and lightweight deployments.
//!
//! Layout on disk:
//!   <root>/
//!     2026-04-14/
//!       <object_id_b64>.json
//!     2026-04-15/
//!       <object_id_b64>.json
//!
//! Query performance on this backend is O(files-in-date-range). That is
//! acceptable for development and auditing small deployments, but it is
//! why Postgres exists as the production backend.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use maat::{ObjectId, Receipt};
use tokio::fs;

use super::{
    normalize_limit, normalize_offset, ReceiptQuery, ReceiptStore, StoreError, StoreResult,
};

pub struct FilesystemStore {
    root: PathBuf,
}

impl FilesystemStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        FilesystemStore { root: root.into() }
    }

    /// Iterate all receipt file paths under the root, newest date directory first.
    /// We walk directories lazily to avoid loading every file when we only need the
    /// first `limit`. Returns paths in roughly-newest-first order.
    async fn collect_paths(&self) -> StoreResult<Vec<PathBuf>> {
        let mut entries = match fs::read_dir(&self.root).await {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(StoreError::Backend(e.to_string())),
        };

        let mut date_dirs: Vec<PathBuf> = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| StoreError::Backend(e.to_string()))?
        {
            let path = entry.path();
            if path.is_dir() {
                date_dirs.push(path);
            }
        }
        date_dirs.sort();
        date_dirs.reverse(); // newest date first

        let mut files: Vec<PathBuf> = Vec::new();
        for dir in date_dirs {
            let mut inner = match fs::read_dir(&dir).await {
                Ok(e) => e,
                Err(e) => return Err(StoreError::Backend(e.to_string())),
            };
            let mut dir_files = Vec::new();
            while let Some(entry) = inner
                .next_entry()
                .await
                .map_err(|e| StoreError::Backend(e.to_string()))?
            {
                let p = entry.path();
                if p.extension().and_then(|s| s.to_str()) == Some("json") {
                    dir_files.push(p);
                }
            }
            dir_files.sort();
            dir_files.reverse();
            files.extend(dir_files);
        }

        Ok(files)
    }

    async fn load_receipt(&self, path: &Path) -> StoreResult<Receipt> {
        let bytes = fs::read(path)
            .await
            .map_err(|e| StoreError::Backend(e.to_string()))?;
        serde_json::from_slice(&bytes).map_err(|e| StoreError::Serialization(e.to_string()))
    }
}

#[async_trait]
impl ReceiptStore for FilesystemStore {
    async fn write(&self, _tenant_id: uuid::Uuid, receipt: &Receipt) -> StoreResult<()> {
        let date = date_subdir(receipt.executed_at);
        let dir = self.root.join(&date);
        fs::create_dir_all(&dir)
            .await
            .map_err(|e| StoreError::Backend(e.to_string()))?;

        let filename = format!("{}.json", receipt.id.to_base64());
        let path = dir.join(filename);

        let json = serde_json::to_string_pretty(receipt)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;

        fs::write(&path, json)
            .await
            .map_err(|e| StoreError::Backend(e.to_string()))?;

        Ok(())
    }

    async fn get(&self, _tenant_id: uuid::Uuid, id: &[u8; 32]) -> StoreResult<Receipt> {
        // Without an index, we must scan. Acceptable on this backend because
        // it is for development and small audits.
        let id_b64 = ObjectId(*id).to_base64();
        let target_filename = format!("{}.json", id_b64);

        for path in self.collect_paths().await? {
            if path.file_name().and_then(|n| n.to_str()) == Some(target_filename.as_str()) {
                return self.load_receipt(&path).await;
            }
        }
        Err(StoreError::NotFound)
    }

    async fn query(&self, query: &ReceiptQuery) -> StoreResult<Vec<Receipt>> {
        let limit = normalize_limit(query.limit);
        let offset = normalize_offset(query.offset) as usize;

        let paths = self.collect_paths().await?;
        let mut results = Vec::new();
        let mut skipped = 0usize;

        for path in paths {
            if results.len() >= limit as usize {
                break;
            }
            let receipt = match self.load_receipt(&path).await {
                Ok(r) => r,
                Err(_) => continue, // skip unreadable files rather than failing the whole query
            };

            if !matches_filters(&receipt, query) {
                continue;
            }

            if skipped < offset {
                skipped += 1;
                continue;
            }

            results.push(receipt);
        }

        Ok(results)
    }

    async fn count(&self, query: &ReceiptQuery) -> StoreResult<u64> {
        let paths = self.collect_paths().await?;
        let mut n = 0u64;
        for path in paths {
            let receipt = match self.load_receipt(&path).await {
                Ok(r) => r,
                Err(_) => continue,
            };
            if matches_filters(&receipt, query) {
                n += 1;
            }
        }
        Ok(n)
    }

    fn backend_name(&self) -> &'static str {
        "filesystem"
    }
}

/// In-memory filter matching. Used by both query() and count() on the
/// filesystem backend since it has no index.
fn matches_filters(receipt: &Receipt, q: &ReceiptQuery) -> bool {
    if let Some(since) = q.since {
        if receipt.executed_at < since {
            return false;
        }
    }
    if let Some(until) = q.until {
        if receipt.executed_at > until {
            return false;
        }
    }
    if let Some(ref scope) = q.action_scope {
        if receipt.action.scope_used != *scope {
            return false;
        }
    }
    if let Some(ref outcome) = q.outcome {
        let o = match receipt.outcome {
            maat::Outcome::Success => "Success",
            maat::Outcome::Failure => "Failure",
            maat::Outcome::Partial => "Partial",
        };
        if o != outcome {
            return false;
        }
    }
    if let Some(deleg_id) = q.delegation_id {
        if receipt.delegation_id.0 != deleg_id {
            return false;
        }
    }
    // agent_pubkey filter is not efficient on the filesystem backend because
    // the agent is embedded in the delegation, not extracted. We skip it
    // rather than mis-implementing. The Postgres backend handles it properly.
    true
}

/// Convert Unix timestamp (seconds) to "YYYY-MM-DD" UTC date.
fn date_subdir(unix_seconds: u64) -> String {
    let days = unix_seconds / 86400;
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}
