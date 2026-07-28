CREATE TABLE IF NOT EXISTS iam.user_statuses(
    id SERIAL PRIMARY KEY,
    status VARCHAR(50)
);

INSERT INTO iam.user_statuses(status)VALUES('Active'), ('Deleted');
