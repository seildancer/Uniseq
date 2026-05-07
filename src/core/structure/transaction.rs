use std::fs;
use std::path::{Path, PathBuf};

use crate::core::{CoreError, FileFingerprint};

use super::OperationKind;
use super::planning::{ExpectedSourceFile, PageMapping, RenameTransactionPlan};

const TRANSACTION_DIR_NAME: &str = ".uniseq-page-transaction";
const MANIFEST_FILE_NAME: &str = "manifest.tsv";
const ORIGINAL_DIR_NAME: &str = "original";
const FINAL_DIR_NAME: &str = "final";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TransactionStatus {
    Prepared,
    Applying,
}

impl TransactionStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Applying => "applying",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "prepared" => Some(Self::Prepared),
            "applying" => Some(Self::Applying),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TransactionBlobEntry {
    workspace_path: PathBuf,
    blob_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TransactionManifest {
    kind: OperationKind,
    status: TransactionStatus,
    page_mappings: Vec<PageMapping>,
    originals: Vec<TransactionBlobEntry>,
    writes: Vec<TransactionBlobEntry>,
    deletes: Vec<PathBuf>,
}

pub(super) struct TransactionRecord {
    manifest: TransactionManifest,
}

impl TransactionRecord {
    pub(super) fn exists(root: &Path) -> bool {
        transaction_dir(root).exists()
    }

    pub(super) fn stage(root: &Path, plan: &RenameTransactionPlan) -> Result<Self, CoreError> {
        let txn_dir = transaction_dir(root);
        if txn_dir.exists() {
            return Err(CoreError::CorruptTransaction);
        }

        let original_texts = load_expected_source_texts(root, &plan.expected_source_files)?;

        let stage_result = (|| -> Result<Self, CoreError> {
            fs::create_dir(&txn_dir).map_err(|error| CoreError::io(&txn_dir, &error))?;
            let originals_dir = txn_dir.join(ORIGINAL_DIR_NAME);
            let finals_dir = txn_dir.join(FINAL_DIR_NAME);
            fs::create_dir(&originals_dir)
                .map_err(|error| CoreError::io(&originals_dir, &error))?;
            fs::create_dir(&finals_dir).map_err(|error| CoreError::io(&finals_dir, &error))?;

            let mut originals = Vec::with_capacity(plan.file_changes.len());
            let mut writes = Vec::with_capacity(plan.file_changes.len());

            for (index, change) in plan.file_changes.iter().enumerate() {
                let original_blob_name = format!("original-{index:04}.blob");
                let final_blob_name = format!("final-{index:04}.blob");
                let original_blob_path = originals_dir.join(&original_blob_name);
                let final_blob_path = finals_dir.join(&final_blob_name);
                let original_text = original_texts
                    .get(&change.original_path)
                    .expect("validated source files are available for every file change");

                fs::write(&original_blob_path, original_text)
                    .map_err(|error| CoreError::io(&original_blob_path, &error))?;
                fs::write(&final_blob_path, &change.final_text)
                    .map_err(|error| CoreError::io(&final_blob_path, &error))?;

                originals.push(TransactionBlobEntry {
                    workspace_path: change.original_path.clone(),
                    blob_name: original_blob_name,
                });
                writes.push(TransactionBlobEntry {
                    workspace_path: change.final_path.clone(),
                    blob_name: final_blob_name,
                });
            }

            let manifest = TransactionManifest {
                kind: plan.kind,
                status: TransactionStatus::Prepared,
                page_mappings: plan.page_mappings.clone(),
                originals,
                writes,
                deletes: plan.deletes.clone(),
            };
            let record = Self { manifest };
            record.write_manifest(root)?;
            Ok(record)
        })();

        if stage_result.is_err() {
            let _ = fs::remove_dir_all(&txn_dir);
        }

        stage_result
    }

    pub(super) fn load(root: &Path) -> Result<Self, CoreError> {
        let manifest_path = transaction_dir(root).join(MANIFEST_FILE_NAME);
        let manifest_text = fs::read_to_string(&manifest_path)
            .map_err(|error| CoreError::io(&manifest_path, &error))?;

        let mut kind = None;
        let mut status = None;
        let mut page_mappings = Vec::new();
        let mut originals = Vec::new();
        let mut writes = Vec::new();
        let mut deletes = Vec::new();

        for line in manifest_text.lines() {
            let fields = line.split('\t').collect::<Vec<_>>();
            match fields.as_slice() {
                ["VERSION", "1"] => {}
                ["KIND", value] => {
                    kind = OperationKind::from_str(value);
                }
                ["STATUS", value] => {
                    status = TransactionStatus::from_str(value);
                }
                ["PAGE_MAP", old_page_id, new_page_id, old_path, new_path] => {
                    page_mappings.push(PageMapping {
                        old_page_id: old_page_id.parse()?,
                        new_page_id: new_page_id.parse()?,
                        old_path: PathBuf::from(old_path),
                        new_path: PathBuf::from(new_path),
                    });
                }
                ["ORIGINAL", workspace_path, blob_name] => originals.push(TransactionBlobEntry {
                    workspace_path: PathBuf::from(workspace_path),
                    blob_name: (*blob_name).to_owned(),
                }),
                ["WRITE", workspace_path, blob_name] => writes.push(TransactionBlobEntry {
                    workspace_path: PathBuf::from(workspace_path),
                    blob_name: (*blob_name).to_owned(),
                }),
                ["DELETE", workspace_path] => deletes.push(PathBuf::from(workspace_path)),
                _ => return Err(CoreError::CorruptTransaction),
            }
        }

        Ok(Self {
            manifest: TransactionManifest {
                kind: kind.ok_or(CoreError::CorruptTransaction)?,
                status: status.ok_or(CoreError::CorruptTransaction)?,
                page_mappings,
                originals,
                writes,
                deletes,
            },
        })
    }

    pub(super) fn mark_applying(&mut self, root: &Path) -> Result<(), CoreError> {
        self.manifest.status = TransactionStatus::Applying;
        self.write_manifest(root)
    }

    pub(super) fn validate_final_paths_available(&self, root: &Path) -> Result<(), CoreError> {
        let txn_dir = transaction_dir(root);
        let original_paths = self
            .manifest
            .originals
            .iter()
            .map(|entry| entry.workspace_path.as_path())
            .collect::<std::collections::BTreeSet<_>>();

        for write in &self.manifest.writes {
            if original_paths.contains(write.workspace_path.as_path()) {
                continue;
            }

            let absolute_path = root.join(&write.workspace_path);
            if !absolute_path.exists() {
                continue;
            }

            let blob_path = txn_dir.join(FINAL_DIR_NAME).join(&write.blob_name);
            let final_text = fs::read_to_string(&blob_path)
                .map_err(|error| CoreError::io(&blob_path, &error))?;
            let existing_text = fs::read_to_string(&absolute_path)
                .map_err(|error| CoreError::io(&absolute_path, &error))?;

            if existing_text != final_text {
                return Err(CoreError::DestinationPageExists);
            }
        }

        Ok(())
    }

    pub(super) fn apply_final_state(
        &self,
        root: &Path,
        write_limit: Option<usize>,
        skip_deletes: bool,
    ) -> Result<(), CoreError> {
        let txn_dir = transaction_dir(root);
        let writes = self
            .manifest
            .writes
            .iter()
            .enumerate()
            .take(write_limit.unwrap_or(self.manifest.writes.len()))
            .collect::<Vec<_>>();

        for (_, write) in writes {
            self.promote_final_write(root, &txn_dir, write)?;
        }

        if skip_deletes {
            return Ok(());
        }

        for delete in &self.manifest.deletes {
            let absolute_path = root.join(delete);
            match fs::remove_file(&absolute_path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(CoreError::io(&absolute_path, &error)),
            }
        }

        self.cleanup_temp_files(root)?;
        Ok(())
    }

    pub(super) fn final_writes(&self, root: &Path) -> Result<Vec<(PathBuf, String)>, CoreError> {
        let txn_dir = transaction_dir(root);
        self.manifest
            .writes
            .iter()
            .map(|write| {
                let blob_path = txn_dir.join(FINAL_DIR_NAME).join(&write.blob_name);
                let final_text = fs::read_to_string(&blob_path)
                    .map_err(|error| CoreError::io(&blob_path, &error))?;
                Ok((write.workspace_path.clone(), final_text))
            })
            .collect()
    }

    pub(super) fn deletes(&self) -> &[PathBuf] {
        &self.manifest.deletes
    }

    pub(super) fn remove(self, root: &Path) -> Result<(), CoreError> {
        self.cleanup_temp_files(root)?;
        let txn_dir = transaction_dir(root);
        fs::remove_dir_all(&txn_dir).map_err(|error| CoreError::io(&txn_dir, &error))
    }

    fn promote_final_write(
        &self,
        root: &Path,
        txn_dir: &Path,
        write: &TransactionBlobEntry,
    ) -> Result<(), CoreError> {
        let blob_path = txn_dir.join(FINAL_DIR_NAME).join(&write.blob_name);
        let final_text =
            fs::read_to_string(&blob_path).map_err(|error| CoreError::io(&blob_path, &error))?;
        let absolute_path = root.join(&write.workspace_path);
        let temp_path = temp_path_for_destination(&absolute_path, &write.blob_name)?;

        if let Ok(existing_text) = fs::read_to_string(&absolute_path) {
            if existing_text == final_text {
                remove_if_exists(&temp_path)?;
                return Ok(());
            }
        }

        if let Some(parent) = absolute_path.parent() {
            fs::create_dir_all(parent).map_err(|error| CoreError::io(parent, &error))?;
        }
        remove_if_exists(&temp_path)?;
        fs::write(&temp_path, &final_text).map_err(|error| CoreError::io(&temp_path, &error))?;

        if absolute_path.exists() {
            fs::remove_file(&absolute_path)
                .map_err(|error| CoreError::io(&absolute_path, &error))?;
        }

        fs::rename(&temp_path, &absolute_path).map_err(|error| CoreError::io(&absolute_path, &error))
    }

    fn cleanup_temp_files(&self, root: &Path) -> Result<(), CoreError> {
        for write in &self.manifest.writes {
            let absolute_path = root.join(&write.workspace_path);
            let temp_path = temp_path_for_destination(&absolute_path, &write.blob_name)?;
            remove_if_exists(&temp_path)?;
        }

        Ok(())
    }

    fn write_manifest(&self, root: &Path) -> Result<(), CoreError> {
        let manifest_path = transaction_dir(root).join(MANIFEST_FILE_NAME);
        let mut lines = vec![
            "VERSION\t1".to_owned(),
            format!("KIND\t{}", self.manifest.kind.as_str()),
            format!("STATUS\t{}", self.manifest.status.as_str()),
        ];

        for mapping in &self.manifest.page_mappings {
            lines.push(format!(
                "PAGE_MAP\t{}\t{}\t{}\t{}",
                mapping.old_page_id.hierarchy_display(),
                mapping.new_page_id.hierarchy_display(),
                path_to_manifest_field(&mapping.old_path)?,
                path_to_manifest_field(&mapping.new_path)?,
            ));
        }

        for entry in &self.manifest.originals {
            lines.push(format!(
                "ORIGINAL\t{}\t{}",
                path_to_manifest_field(&entry.workspace_path)?,
                entry.blob_name
            ));
        }

        for entry in &self.manifest.writes {
            lines.push(format!(
                "WRITE\t{}\t{}",
                path_to_manifest_field(&entry.workspace_path)?,
                entry.blob_name
            ));
        }

        for delete in &self.manifest.deletes {
            lines.push(format!("DELETE\t{}", path_to_manifest_field(delete)?));
        }

        let manifest_text = lines.join("\n");
        fs::write(&manifest_path, manifest_text)
            .map_err(|error| CoreError::io(&manifest_path, &error))
    }
}

fn load_expected_source_texts(
    root: &Path,
    expected_source_files: &[ExpectedSourceFile],
) -> Result<std::collections::BTreeMap<PathBuf, String>, CoreError> {
    let mut original_texts = std::collections::BTreeMap::new();

    for expected in expected_source_files {
        let absolute_path = root.join(&expected.workspace_path);
        let disk_text = match fs::read_to_string(&absolute_path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(CoreError::StructuralConflict {
                    path: expected.workspace_path.clone(),
                });
            }
            Err(error) => return Err(CoreError::io(&absolute_path, &error)),
        };

        if FileFingerprint::from_text(&disk_text) != expected.fingerprint {
            return Err(CoreError::StructuralConflict {
                path: expected.workspace_path.clone(),
            });
        }

        original_texts.insert(expected.workspace_path.clone(), disk_text);
    }

    Ok(original_texts)
}

fn path_to_manifest_field(path: &Path) -> Result<String, CoreError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or(CoreError::CorruptTransaction)
}

fn transaction_dir(root: &Path) -> PathBuf {
    root.join(TRANSACTION_DIR_NAME)
}

fn temp_path_for_destination(
    destination_path: &Path,
    blob_name: &str,
) -> Result<PathBuf, CoreError> {
    let file_name = destination_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(CoreError::CorruptTransaction)?;
    Ok(destination_path.with_file_name(format!(
        "{file_name}.uniseq-txn-{blob_name}.tmp"
    )))
}

fn remove_if_exists(path: &Path) -> Result<(), CoreError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CoreError::io(path, &error)),
    }
}
