CREATE TABLE hosted_service_metadata (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    profile TEXT NOT NULL CHECK (profile = 'hosted-standard-v1'),
    schema_version INTEGER NOT NULL CHECK (schema_version = 1)
) STRICT;

INSERT INTO hosted_service_metadata (singleton, profile, schema_version)
VALUES (1, 'hosted-standard-v1', 1);
