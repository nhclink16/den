ALTER TABLE settings ADD COLUMN instance_name TEXT NOT NULL DEFAULT 'Den' CHECK(length(instance_name) BETWEEN 1 AND 40);
