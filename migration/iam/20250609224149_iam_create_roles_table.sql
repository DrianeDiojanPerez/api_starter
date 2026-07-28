-- migrate:up
CREATE TABLE IF NOT EXISTS iam.roles(
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL UNIQUE
);

INSERT INTO iam.roles(name) VALUES ('Admin'), ('Developer'), ('Staff');

-- migrate:down
DROP TABLE IF EXISTS iam.roles;
