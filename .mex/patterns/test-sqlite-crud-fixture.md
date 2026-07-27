---
name: test-sqlite-crud-fixture
description: Exercise SQLite model CRUD without changing the active library configuration.
last_updated: 2026-07-27
---

# SQLite CRUD Fixture

Use an explicit connection-backed SQL core for model methods that normally open the current library connection. Keep the public/production wrapper unchanged, and test the core with a per-test temporary SQLite file.

## Fixture rules

- Create only the tables and columns required by the SQL under test, including foreign keys and unique constraints that production migrations provide.
- Build media fixtures through the existing model constructor when possible; use a small temporary source file and a non-decoding file type to avoid requiring bundled metadata assets.
- Test metadata-read guards with deliberately invalid bytes before adding binary EXIF fixtures; this verifies routing without coupling CRUD tests to decoder behavior.
- Build minimal little-endian TIFF bytes in the test for deterministic EXIF coverage. Use IFD0 for Make/Model/Software and an ExifIFD pointer for DateTimeOriginal; assert both parsed identity and modified-time fallback.
- Verify the database row after insert and update, then verify dependent rows (for example thumbnails) are cleaned during delete.
- Always drop the connection and remove both database and source fixture paths, including on assertion-free success paths.

## Verify

```text
cargo test t_sqlite:: --manifest-path src-tauri/Cargo.toml
```

Keep full metadata extraction fixtures as a separate follow-up; do not expand a CRUD fixture into an `AFile::new` integration suite by accident.
