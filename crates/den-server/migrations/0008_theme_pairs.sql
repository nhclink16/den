-- The startup companion completes null halves in a transaction using den-core's
-- OKLCH derivation before any request is served. Nulls are a restart-safe marker:
-- a crash between this SQL migration and the companion loses no authored colors.
UPDATE user_appearance SET appearance = json_object(
    'mode', json_extract(appearance, '$.mode'),
    'theme', CASE coalesce(json_extract(appearance, '$.dark_theme'), json_extract(appearance, '$.light_theme'), 'den')
        WHEN 'den-light' THEN 'den'
        ELSE coalesce(json_extract(appearance, '$.dark_theme'), json_extract(appearance, '$.light_theme'), 'den') END,
    'custom_themes', json(coalesce((SELECT json_group_array(json_object(
        'id', json_extract(value, '$.id'), 'name', json_extract(value, '$.name'),
        'light', json(CASE json_extract(value, '$.appearance') WHEN 'light' THEN json_extract(value, '$.colors') END),
        'dark', json(CASE json_extract(value, '$.appearance') WHEN 'dark' THEN json_extract(value, '$.colors') END),
        'fonts', json_extract(value, '$.fonts'), 'radius', json_extract(value, '$.radius'), 'density', json_extract(value, '$.density')
    )) FROM json_each(appearance, '$.custom_themes')), '[]'))
) WHERE json_type(appearance, '$.theme') IS NULL;
