CREATE TABLE IF NOT EXISTS iam.password_resets(
   email VARCHAR(255) NOT NULL,
   token VARCHAR(255) NOT NULL,
   created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX iam_password_resets_email_index ON iam.password_resets USING btree(email);
CREATE INDEX iam_password_resets_token_index ON iam.password_resets USING btree(token);
