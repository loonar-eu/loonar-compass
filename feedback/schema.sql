-- The D1 database of the feedback service (npx wrangler d1 execute compass-stats --remote --file schema.sql).
-- No addresses (IP), no names: the random installation id, versions and counters only.

CREATE TABLE IF NOT EXISTS starts (
  install TEXT NOT NULL,
  day     TEXT NOT NULL,
  app     TEXT NOT NULL,
  os      TEXT NOT NULL,
  lang    TEXT NOT NULL,
  at      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS starts_day ON starts (day);
CREATE INDEX IF NOT EXISTS starts_install ON starts (install);

-- The newest usage summary of a day of an installation (partial = 1 while the day goes on).
CREATE TABLE IF NOT EXISTS days (
  install TEXT NOT NULL,
  day     TEXT NOT NULL,
  app     TEXT NOT NULL,
  partial INTEGER NOT NULL,
  data    TEXT NOT NULL,
  updated TEXT NOT NULL,
  PRIMARY KEY (install, day)
);
CREATE INDEX IF NOT EXISTS days_day ON days (day);

-- The summary of a week over all installations together (computed on Mondays; GET /v1/summary).
CREATE TABLE IF NOT EXISTS weekly (
  week    TEXT PRIMARY KEY,
  data    TEXT NOT NULL,
  created TEXT NOT NULL
);
