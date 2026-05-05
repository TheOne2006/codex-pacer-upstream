SELECT session_id, timestamp, model_id, input_tokens, cached_input_tokens,
       output_tokens, reasoning_output_tokens, total_tokens, value_usd
FROM usage_events
ORDER BY timestamp ASC, id ASC
