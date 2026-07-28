-- migrate:up
CREATE SCHEMA IF NOT EXISTS iam;

-- migrate:down
DROP SCHEMA iam;
