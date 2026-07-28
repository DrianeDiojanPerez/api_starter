CREATE TABLE IF NOT EXISTS iam.departments(
    id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    company_id INT,
    FOREIGN KEY (company_id) REFERENCES iam.companies(id)
);


INSERT INTO iam.departments (name, company_id) VALUES ('Administration', 1);
