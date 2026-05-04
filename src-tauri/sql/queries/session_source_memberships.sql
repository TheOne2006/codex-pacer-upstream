SELECT session_id, source_id
FROM import_state
WHERE session_id IS NOT NULL AND source_id IS NOT NULL
