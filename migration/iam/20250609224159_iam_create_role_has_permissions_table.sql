-- migrate:up
CREATE TABLE IF NOT EXISTS iam.role_has_permissions(
    role_id INT,
    permission_id INT,
    PRIMARY KEY (role_id, permission_id),
    FOREIGN KEY (role_id) REFERENCES iam.roles(id) ON DELETE CASCADE,
    FOREIGN KEY (permission_id) REFERENCES iam.permissions(id) ON DELETE CASCADE
);

INSERT INTO iam.role_has_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM iam.roles r, iam.permissions p
WHERE r.name = 'Admin' OR r.name = 'Developer';

-- migrate:down
DROP TABLE IF EXISTS iam.role_has_permissions;
