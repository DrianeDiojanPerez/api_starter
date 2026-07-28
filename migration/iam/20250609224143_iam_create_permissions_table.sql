-- migrate:up
CREATE TABLE
    IF NOT EXISTS iam.permissions (
        id SERIAL PRIMARY KEY,
        name varchar,
        resource varchar,
        module_id INT,
        FOREIGN KEY(module_id) REFERENCES iam.modules(id)
    );

INSERT INTO
    iam.permissions (name, resource, module_id)
VALUES
    ('View All', 'Users', 1),
    ('Delete', 'Catalogues', 2),
    ('Publish', 'Catalogues', 2),
    ('Add Tags', 'Catalogues', 2),
    ('Make A Copy', 'Catalogues', 2),
    ('Restore', 'Catalogues', 2),
    ('Delete', 'Customers', 3);
-- migrate:down
DROP TABLE IF EXISTS iam.permissions;
