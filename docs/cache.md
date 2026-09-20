# Bounded schema cache

`flagpick::cache::SchemaCache` stores validated `SchemaDocument` records as local JSON files. It is a library foundation for the future discovery pipeline; there is no public cache command yet.

## Location

The documented default is:

- `$XDG_CACHE_HOME/flagpick/` when `XDG_CACHE_HOME` is set;
- otherwise `~/.cache/flagpick/` when `HOME` is available.

Callers may provide an explicit directory to `SchemaCache::new`.

## Identity and invalidation

`CacheKey::from_executable` requires an absolute executable path and hashes the executable bytes with SHA-256. The key also includes the invoked alias/subcommand vector, parser version, and schema version. Any change produces a different cache filename and therefore a miss.

Records use a versioned `v1-<sha256>.json` filename and contain only the cache version, key, and validated schema document. Shell buffers, probe output, credentials, and environment variables are not cache inputs.

## Safety behavior

- Each record is bounded to 16 MiB.
- Writes use an exclusive temporary file in the cache directory, then rename it into place.
- Corrupt, oversized, mismatched, or invalid records are removed and treated as misses.
- Permission-denied reads fail open as misses so discovery can continue without cache access.
- `clear` removes only files matching the cache filename grammar; it does not recursively delete the caller-provided root.
- `list` validates records and returns their keys, removing invalid records.

The cache is not a security boundary or a database. It is an optimization for validated local metadata.
