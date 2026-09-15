-- Search provider credentials were historically serialized inside app_settings.settings.
-- They cannot be considered safe after plaintext persistence, so remove them and require
-- users to rotate/reconnect through the secure credential flow.
UPDATE app_settings
SET settings = jsonb_set(
  (settings #- '{search,apiKey}') #- '{search,api_key}',
  '{search,authConfigured}',
  'false'::jsonb,
  true
)
WHERE settings #> '{search}' IS NOT NULL;
