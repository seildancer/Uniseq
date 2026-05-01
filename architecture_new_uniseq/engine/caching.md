# Caching Strategy

Caching should follow an Obsidian-like benchmark.

That means:

- markdown files are canonical
- metadata and search caches exist for speed
- caches are persisted locally
- caches are disposable and rebuildable
- sync operates on files, not on cache state

## What should be cached

- parsed file projections
- page registry
- incoming reference index
- search index
- graph edge summaries
- suggestion and ranking caches

## What should be eager

- parse the file currently being edited or saved
- update outbound refs for the changed file
- update local search data needed for immediate UI feedback

## What can be lazy

- backlinks for unopened target pages
- graph neighborhoods away from the active context
- secondary ranking and suggestion caches
- expensive cross-workspace aggregations

## What can be stale

Only disposable derived state may be stale:

- cached backlink counts
- cached graph weights
- cached suggestion results

Markdown content and authored links must not depend on cache freshness.

## Rebuild rule

The app must expose a safe rebuild path for all caches.

Rebuilding may take time, but it must be deterministic and must not require user cleanup.
