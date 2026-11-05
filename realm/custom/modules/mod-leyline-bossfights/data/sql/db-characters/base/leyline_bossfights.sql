-- mod-leyline-bossfights (characters database). LeylineBossfightsWorldScript::OnStartup creates the same tables if
-- missing, and realm/custom/bossfights-export.py --schema prints the same text (stream/tests/test_bossfights_export.py
-- keeps the copies equal).

-- One row per attempt: a clear, a wipe, or an abandon (the raid left while the boss was fighting).
-- ms: from the boss engaging to the start of its 10% cast (clear), its reset (wipe) or the leaving (abandon).
-- roster: "Name:class:bot|player", comma separated, everyone in the instance when the boss engaged.
-- gm 1: a GM was in the raid or a member used a GM command; such runs never reach the boards.
CREATE TABLE IF NOT EXISTS `leyline_bossfight_runs` (
  `id` INT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
  `boss` VARCHAR(32) NOT NULL,
  `difficulty` VARCHAR(8) NOT NULL,
  `mode` VARCHAR(12) NOT NULL DEFAULT 'bots',
  `size` TINYINT UNSIGNED NOT NULL DEFAULT 0,
  `ms` INT UNSIGNED NOT NULL DEFAULT 0,
  `started_at` INT UNSIGNED NOT NULL DEFAULT 0,
  `ended_at` INT UNSIGNED NOT NULL DEFAULT 0,
  `leader_guid` INT UNSIGNED NOT NULL DEFAULT 0,
  `instance_id` INT UNSIGNED NOT NULL DEFAULT 0,
  `roster` TEXT NOT NULL,
  `bots` TINYINT UNSIGNED NOT NULL DEFAULT 0,
  `bot_buffs` TINYINT UNSIGNED NOT NULL DEFAULT 0,
  `gm` TINYINT UNSIGNED NOT NULL DEFAULT 0,
  `tier` TINYINT UNSIGNED NOT NULL DEFAULT 0,
  `wipes_before` SMALLINT UNSIGNED NOT NULL DEFAULT 0,
  `result` VARCHAR(8) NOT NULL,
  KEY `board` (`boss`, `difficulty`, `result`, `ms`),
  KEY `ended` (`ended_at`)
);

-- The instances the module made (kept 14 days): a player who logs in inside one with no live run is sent home.
CREATE TABLE IF NOT EXISTS `leyline_bossfight_instances` (
  `instance_id` INT UNSIGNED NOT NULL PRIMARY KEY,
  `boss` VARCHAR(32) NOT NULL,
  `created_at` INT UNSIGNED NOT NULL DEFAULT 0
);
