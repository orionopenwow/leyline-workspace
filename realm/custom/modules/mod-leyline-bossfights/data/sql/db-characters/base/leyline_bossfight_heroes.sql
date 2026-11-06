-- mod-leyline-bossfights: boss fight heroes (docs/BOSS-FIGHT-HEROES.md). The module makes the same tables at startup.
CREATE TABLE IF NOT EXISTS `leyline_bf_hero_accounts` (
  `hero_account_id` INT UNSIGNED NOT NULL PRIMARY KEY,
  `main_account_id` INT UNSIGNED NOT NULL,
  `current_guid` INT UNSIGNED NOT NULL DEFAULT 0,
  `last_class` TINYINT UNSIGNED NOT NULL DEFAULT 0,
  `last_spec` VARCHAR(12) NOT NULL DEFAULT '',
  `last_spec2` VARCHAR(12) NOT NULL DEFAULT '',
  `last_faction` TINYINT NOT NULL DEFAULT -1,
  `last_go` VARCHAR(40) NOT NULL DEFAULT 'bf:frozen-throne:normal:bots',
  `pending_go` VARCHAR(40) NOT NULL DEFAULT '',
  `pending_until` INT UNSIGNED NOT NULL DEFAULT 0,
  UNIQUE KEY `main` (`main_account_id`)
);

CREATE TABLE IF NOT EXISTS `leyline_bf_heroes` (
  `guid` INT UNSIGNED NOT NULL PRIMARY KEY,
  `hero_account_id` INT UNSIGNED NOT NULL,
  `class` TINYINT UNSIGNED NOT NULL,
  `spec` VARCHAR(12) NOT NULL,
  `spec2` VARCHAR(12) NOT NULL,
  `created_at` INT UNSIGNED NOT NULL DEFAULT 0,
  KEY `account` (`hero_account_id`)
);

-- One row the module refreshes every minute: ov_go.lua reads it before starting a "bf:" go.
CREATE TABLE IF NOT EXISTS `leyline_bossfight_live` (
  `id` TINYINT UNSIGNED NOT NULL PRIMARY KEY,
  `version` INT UNSIGNED NOT NULL DEFAULT 0,
  `beat` INT UNSIGNED NOT NULL DEFAULT 0
);
