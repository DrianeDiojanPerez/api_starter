-- migrate:up
CREATE TABLE IF NOT EXISTS iam.user_has_roles(
    user_id uuid,
    role_id INT,
    PRIMARY KEY(user_id, role_id),
    FOREIGN KEY (user_id)  REFERENCES iam.users(id) ON DELETE CASCADE,
    FOREIGN KEY (role_id) REFERENCES iam.roles(id)
);

INSERT INTO iam.user_has_roles(user_id, role_id)
SELECT u.id, r.id
FROM iam.users u, iam.roles r
WHERE r.name = 'Admin';

-- migrate:down
DROP TABLE IF EXISTS iam.user_has_roles;
