-- Setup script to create source tables and populate with test data

-- Create raw schema
CREATE SCHEMA IF NOT EXISTS raw;

-- Create and populate users table
CREATE TABLE IF NOT EXISTS raw.users (
    user_id INTEGER,
    user_name VARCHAR,
    signup_date DATE
);

DELETE FROM raw.users;

INSERT INTO raw.users VALUES
    (1, 'Alice', '2025-01-01'),
    (2, 'Bob', '2025-01-02'),
    (3, 'Charlie', '2025-01-03'),
    (4, 'Diana', '2025-01-04'),
    (5, 'Eve', '2025-01-05');

-- Create and populate events table
CREATE TABLE IF NOT EXISTS raw.events (
    event_id INTEGER,
    user_id INTEGER,
    event_type VARCHAR,
    event_timestamp TIMESTAMP
);

DELETE FROM raw.events;

INSERT INTO raw.events VALUES
    (1, 1, 'login', '2025-01-10 08:00:00'),
    (2, 1, 'page_view', '2025-01-10 08:05:00'),
    (3, 2, 'login', '2025-01-10 09:00:00'),
    (4, 1, 'purchase', '2025-01-10 10:30:00'),
    (5, 3, 'login', '2025-01-10 11:00:00'),
    (6, 2, 'page_view', '2025-01-10 11:15:00'),
    (7, 3, 'page_view', '2025-01-10 11:20:00'),
    (8, 1, 'logout', '2025-01-10 12:00:00'),
    (9, 4, 'login', '2025-01-10 13:00:00'),
    (10, 2, 'purchase', '2025-01-10 14:00:00');
