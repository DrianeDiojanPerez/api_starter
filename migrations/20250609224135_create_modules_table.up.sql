CREATE TABLE IF NOT EXISTS iam.modules(
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL
);

INSERT INTO iam.modules (name) VALUES ('IAM Module'), ('Catalogues Module'), ('Customer Module');
