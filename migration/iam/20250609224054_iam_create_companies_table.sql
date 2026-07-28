-- migrate:up
CREATE TABLE IF NOT EXISTS iam.companies(
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) UNIQUE NOT NULL
);

INSERT INTO iam.companies(name) VALUES ('Example Company Ltd');

-- migrate:down
DROP TABLE IF EXISTS iam.companies;
