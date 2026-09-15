-- Preserve each existing family for both independently selectable halves.
UPDATE user_appearance
SET appearance = json_set(
    json_remove(appearance, '$.theme'),
    '$.light_theme', COALESCE(json_extract(appearance, '$.theme'), 'den'),
    '$.dark_theme', COALESCE(json_extract(appearance, '$.theme'), 'den'),
    '$.background', NULL,
    '$.contrast', 100
);
