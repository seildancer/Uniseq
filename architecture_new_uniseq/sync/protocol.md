# Sync Protocol

The sync protocol should operate over files, manifests, hashes, and patches.

## Core objects

- workspace ID
- device ID
- file path
- file hash
- modified timestamp
- revision marker
- encrypted content or patch payload
- conflict record

## Sync passes

- upload local changes
- download remote changes
- reconcile manifests
- detect conflicts
- apply safe remote updates
- leave conflict files when automatic merge is unsafe

## Conflict model

Because there is no collaboration target, conflict handling can be conservative and file-level.

When the same file changes on two devices:

- attempt a three-way merge
- prefer append-safe journal merges where possible
- create a conflict file when uncertain
- surface conflicts in the UI

The sync layer does not need entity-level or block-level merge semantics.
