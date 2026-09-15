ALTER TABLE users ADD COLUMN banner_url TEXT;
ALTER TABLE users ADD COLUMN bio TEXT;
ALTER TABLE users ADD COLUMN accent TEXT;
ALTER TABLE users ADD COLUMN status TEXT CHECK(status IS NULL OR json_valid(status));
