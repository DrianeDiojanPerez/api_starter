-- migrate:up
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE IF NOT EXISTS iam.users(
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_name VARCHAR(100) UNIQUE NOT NULL,
    avatar_id VARCHAR(255) NOT NULL DEFAULT '',
    email VARCHAR(100) UNIQUE NOT NULL,
    password VARCHAR(255) NOT NULL,
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    status_id INT NOT NULL,
    department_id INT NOT NULL,
    FOREIGN KEY (department_id) REFERENCES iam.departments(id),
    FOREIGN KEY (status_id) REFERENCES iam.user_statuses(id)
);

INSERT INTO iam.users(user_name,email, password,first_name, last_name,status_id, department_id) VALUES('admin', 'admin@example.com', '$2a$10$Mseb73NYJsJ/d8fcD.JZGuJgAeciQXFxaxY7uXdVCtvH2u/muejNy', 'App', 'Admin', 1, 1);
-- migrate:down
DROP TABLE IF EXISTS iam.users;
