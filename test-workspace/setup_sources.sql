-- Create source schema
CREATE SCHEMA IF NOT EXISTS source;

-- Create source.events table with sample data
CREATE TABLE IF NOT EXISTS source.events AS
SELECT
    row_number() OVER () as event_id,
    (row_number() OVER () % 10) + 1 as user_id,
    TIMESTAMP '2024-01-01 00:00:00' + INTERVAL (row_number() OVER ()) HOUR as event_time,
    CASE (row_number() OVER () % 3)
        WHEN 0 THEN 'page_view'
        WHEN 1 THEN 'click'
        ELSE 'scroll'
    END as event_type
FROM generate_series(1, 100);
