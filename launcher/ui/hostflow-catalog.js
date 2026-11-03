// Leyline: the Host page's settings and templates (hostflow.js), one list for the page and the core. The launcher's Rust
// core reads this same file (src-tauri/src/host/flow.rs, include_str!) to check and apply a plan, so the page and the
// core can't drift: keep it plain JSON after the "=" (no comments inside, no trailing commas).
//
// Every setting maps to something the realm really does (docs/leyline/host-a-realm.md, "Every setting"):
//   env      world server settings in state/mode.json "resolved" (setup.py read_mode -> state/realm.env): only the
//            names a mode may set (OV_*, AC_RATE_*, AC_SKILL_GAIN_*, AC_START_*, AC_AUTO_BALANCE_*)
//   option   state/options.json, read once by setup.py init (check_options: type, bots, max, signup)
//   world    the ruleset (realm/profiles/<name>.env)
//   rule     a line in the house rules (state/rules.txt; the realm list shows them)
//   mods     the realm's suggested addons and game modes (catalog ids, the realm list's mods)
//   coming   shown, never applied: the Host pack doesn't set it yet
// "only": the worlds (rulesets) it works on; "needs": another setting that must be on.
window.LeylineHostCatalog = {
  "version": 1,
  "groups": [
    { "id": "levels", "label": "Levels" },
    { "id": "rates", "label": "Rates" },
    { "id": "world", "label": "World" },
    { "id": "players", "label": "Players" },
    { "id": "rules", "label": "Rules" },
    { "id": "addons", "label": "Addons and modes" }
  ],
  "worlds": [
    { "id": "whole", "ruleset": "main", "label": "The whole world, to 80", "max_level": 80 },
    { "id": "eras", "ruleset": "progression", "label": "Every era in turn, to 80", "max_level": 80 },
    { "id": "first", "ruleset": "vanillaplus", "label": "The first era only, to 60", "max_level": 60 },
    { "id": "hangout", "ruleset": "dalaran", "label": "A light hangout in Dalaran", "max_level": 80 },
    { "id": "top", "ruleset": "brawl", "label": "Everyone starts at 80", "max_level": 80 },
    { "id": "solo", "ruleset": "solo", "label": "Just me, with helper bots", "max_level": 80, "private_only": true }
  ],
  "settings": [
    { "key": "world", "group": "levels", "label": "World and top level", "kind": "choice", "maps": "world",
      "options": [["whole", "The whole world, to 80"], ["eras", "Every era in turn, to 80"], ["first", "The first era only, to 60"],
        ["hangout", "A light hangout in Dalaran"], ["top", "Everyone starts at 80"], ["solo", "Just me, with helper bots"]],
      "help": "Picks the realm's ruleset. Just me is private only." },
    { "key": "boost", "group": "levels", "label": "One-time boost to 80", "kind": "toggle", "maps": "env", "env": ["OV_MAIN"],
      "only": ["whole", "hangout"], "help": "A new character may jump to 80 with a gear kit, or start at 1." },
    { "key": "mount", "group": "levels", "label": "Riding and a mount from level 1", "kind": "toggle", "maps": "env", "env": ["OV_START_MOUNT"],
      "only": ["whole", "hangout"], "needs": "boost", "help": "Comes with the boost choice." },
    { "key": "start_gold", "group": "levels", "label": "Starting gold", "kind": "range", "min": 0, "max": 1000, "step": 10, "unit": "gold",
      "maps": "env", "env": ["AC_START_PLAYER_MONEY"], "scale": 10000 },
    { "key": "autolearn", "group": "levels", "label": "Learn class spells on level up", "kind": "toggle", "maps": "env", "env": ["OV_AUTOLEARN"],
      "help": "Off: visit trainers, as in the original game." },
    { "key": "start_level", "group": "levels", "label": "Any start level", "kind": "coming",
      "help": "Coming. Today: level 1, the boost, or Everyone starts at 80." },

    { "key": "xp", "group": "rates", "label": "Experience", "kind": "range", "min": 0.5, "max": 10, "step": 0.5, "unit": "x",
      "maps": "env", "env": ["AC_RATE_XP_KILL", "AC_RATE_XP_QUEST", "AC_RATE_XP_QUEST_DF", "AC_RATE_XP_EXPLORE"] },
    { "key": "rested", "group": "rates", "label": "Rested bonus builds", "kind": "range", "min": 1, "max": 4, "step": 1, "unit": "x",
      "maps": "env", "env": ["AC_RATE_REST_IN_GAME", "AC_RATE_REST_OFFLINE_IN_TAVERN_OR_CITY", "AC_RATE_REST_OFFLINE_IN_WILDERNESS"] },
    { "key": "drops", "group": "rates", "label": "Item drops", "kind": "range", "min": 1, "max": 5, "step": 0.5, "unit": "x",
      "maps": "env", "env": ["AC_RATE_DROP_ITEM_POOR", "AC_RATE_DROP_ITEM_NORMAL", "AC_RATE_DROP_ITEM_UNCOMMON", "AC_RATE_DROP_ITEM_RARE", "AC_RATE_DROP_ITEM_EPIC"] },
    { "key": "gold", "group": "rates", "label": "Gold from monsters", "kind": "range", "min": 1, "max": 5, "step": 0.5, "unit": "x",
      "maps": "env", "env": ["AC_RATE_DROP_MONEY"] },
    { "key": "rep", "group": "rates", "label": "Reputation", "kind": "range", "min": 1, "max": 5, "step": 0.5, "unit": "x",
      "maps": "env", "env": ["AC_RATE_REPUTATION_GAIN"] },
    { "key": "honor", "group": "rates", "label": "Honor", "kind": "range", "min": 1, "max": 5, "step": 0.5, "unit": "x",
      "maps": "env", "env": ["AC_RATE_HONOR"] },
    { "key": "skill", "group": "rates", "label": "Profession skill-ups", "kind": "range", "min": 1, "max": 5, "step": 1, "unit": "pt",
      "maps": "env", "env": ["AC_SKILL_GAIN_CRAFTING", "AC_SKILL_GAIN_GATHERING"], "help": "Skill points per skill-up." },

    { "key": "bots", "group": "world", "label": "Roaming bots", "kind": "range", "min": 0, "max": 200, "step": 5, "unit": "bots",
      "maps": "option", "option": "bots", "only": ["whole", "eras", "first", "top", "solo"],
      "help": "Computer players who quest and queue. Each one uses some of your PC." },
    { "key": "helpers", "group": "world", "label": "Helper bots per player", "kind": "range", "min": 1, "max": 4, "step": 1, "unit": "bots",
      "maps": "env", "env": ["OV_COMPANIONS_PER_PLAYER"], "only": ["solo"], "help": "Type .recruit party in the game to fill your group." },
    { "key": "scaling", "group": "world", "label": "Dungeons scale to your group", "kind": "toggle", "maps": "env", "env": ["AC_AUTO_BALANCE_ENABLE_GLOBAL"] },
    { "key": "realm_type", "group": "world", "label": "Realm type", "kind": "choice", "maps": "option", "option": "type",
      "options": [["normal", "PvE: fight when you choose"], ["pvp", "PvP: open-world fights"], ["rp", "RP: role-play, PvE"], ["rppvp", "RP-PvP"]] },
    { "key": "duel_reset", "group": "world", "label": "Health and cooldowns back after a duel", "kind": "toggle", "maps": "env", "env": ["OV_DUEL_RESET"] },
    { "key": "duel_ladder", "group": "world", "label": "Duel ladder (ratings, weekly points)", "kind": "toggle", "maps": "env", "env": ["OV_DUELS"],
      "help": "Level 80 duels count. .duel top in the game." },
    { "key": "events", "group": "world", "label": "Events clock", "kind": "toggle", "maps": "env", "env": ["OV_EVENTS", "OV_EVENTS_STOCK"],
      "help": "Holidays and contests, announced 5 minutes ahead." },
    { "key": "trivia", "group": "world", "label": "Trivia every other hour", "kind": "toggle", "maps": "env", "env": ["OV_EVENTS_TRIVIA"], "needs": "events" },
    { "key": "hide", "group": "world", "label": "Hide and seek in Dalaran", "kind": "toggle", "maps": "env", "env": ["OV_EVENTS_HIDE"], "needs": "events" },
    { "key": "hubs", "group": "world", "label": "Teleports to the hubs (.hub)", "kind": "toggle", "maps": "env", "env": ["OV_QOL_HUB"] },
    { "key": "free_repair", "group": "world", "label": "Free repairs", "kind": "toggle", "maps": "env", "env": ["OV_FREE_REPAIR"] },
    { "key": "weather", "group": "world", "label": "Weather and time of day", "kind": "coming", "help": "Change them live in the GM command center. A realm-wide default is coming." },

    { "key": "max_players", "group": "players", "label": "Player limit", "kind": "range", "min": 0, "max": 500, "step": 5, "unit": "players",
      "maps": "option", "option": "max", "help": "0 = the ruleset's own. When it's full, players wait in a queue." },
    { "key": "signup", "group": "players", "label": "Who can make an account", "kind": "choice", "maps": "option", "option": "signup",
      "options": [["invite", "Only people with my invite link"], ["open", "Anyone, on my join page"], ["off", "Nobody new for now"]] },

    { "key": "rp_names", "group": "rules", "label": "Names that fit the world", "kind": "toggle", "maps": "rule",
      "rule": "Role-play realm: character names fit the world. No joke names; stay in character in public chat." },
    { "key": "one_life", "group": "rules", "label": "Hardcore challenge welcome", "kind": "toggle", "maps": "rule",
      "rule": "Hardcore welcome: one life. Start at level 1 and touch the Shrine of Challenge by your start zone's graveyard.",
      "help": "Each player picks one life for a character at the Shrine. Forcing it on everyone is coming." },
    { "key": "chat_filter", "group": "rules", "label": "Star out bad words in public chat", "kind": "toggle", "maps": "env", "env": ["OV_NAMES_CHAT"] },
    { "key": "chat_level", "group": "rules", "label": "Level needed to talk in channels", "kind": "coming", "help": "Coming." },

    { "key": "loadout", "group": "addons", "label": "Addons the realm suggests", "kind": "choice", "maps": "mods",
      "options": [["none", "None"], ["quester", "Quester"], ["raider", "Raider"], ["clean-frames", "Tidy interface"]],
      "help": "Players are offered them when they join. Only reviewed catalog addons." },
    { "key": "minigolf", "group": "addons", "label": "Minigolf, Bumper Balls and Coin Dash", "kind": "toggle", "maps": "mods",
      "only": ["hangout"], "help": "The game modes players built, in the hangout." }
  ],
  "loadouts": {
    "none": [],
    "quester": ["up:questie", "up:gathermate", "up:idtip", "up:ratingbuster"],
    "raider": ["up:skada", "up:tellmewhen", "up:weakauras-bunny67", "up:sharedmedia", "up:blt", "up:krt", "up:gearscorelite"],
    "clean-frames": ["up:dominos", "up:adibags", "up:ruf", "up:platebuffs", "up:pretty-minimap"]
  },
  "minigolf_mods": ["mode:minigolf", "mode:bumper-balls", "mode:coin-dash"],
  "base": {
    "world": "whole", "boost": true, "mount": true, "start_gold": 0, "autolearn": true,
    "xp": 3, "rested": 2, "drops": 1, "gold": 1, "rep": 2, "honor": 1, "skill": 2,
    "bots": 40, "helpers": 4, "scaling": true, "realm_type": "normal", "duel_reset": true, "duel_ladder": true,
    "events": true, "trivia": true, "hide": true, "hubs": false, "free_repair": false,
    "max_players": 0, "signup": "invite",
    "rp_names": false, "one_life": false, "chat_filter": true,
    "loadout": "none", "minigolf": false
  },
  "templates": [
    { "id": "pure-vanilla", "name": "Pure vanilla", "best_for": "Purists",
      "pitch": "The first era as it was: the original rules, 1x everything, to level 60.",
      "set": { "world": "first", "boost": false, "mount": false, "autolearn": false, "xp": 1, "rested": 1, "drops": 1, "gold": 1, "rep": 1,
        "honor": 1, "skill": 1, "bots": 0, "scaling": false, "duel_reset": false, "duel_ladder": false, "events": true, "trivia": false,
        "hide": false } },
    { "id": "vanilla-plus", "name": "Vanilla+", "best_for": "Friends with jobs",
      "pitch": "The first era with comforts: 1.5x, faster rested, spells on level up, dungeons that fit your group.",
      "set": { "world": "first", "boost": false, "mount": false, "autolearn": true, "xp": 1.5, "rested": 2, "rep": 1.5, "skill": 2, "bots": 0,
        "scaling": true, "duel_ladder": false, "trivia": false, "hide": false, "loadout": "quester" } },
    { "id": "original", "name": "Original rules, every era", "best_for": "The long road",
      "pitch": "1x from level 1. Each character moves through the eras in turn, at their own pace.",
      "set": { "world": "eras", "boost": false, "mount": false, "autolearn": false, "xp": 1, "rested": 1, "rep": 1, "skill": 1, "bots": 20,
        "scaling": false, "duel_ladder": false, "trivia": false, "hide": false } },
    { "id": "classic", "name": "Classic levelling", "best_for": "Most groups",
      "pitch": "Level 1 to 80 at 3x, with a one-time boost for alts and a mount from level 1.",
      "set": {} },
    { "id": "fun", "name": "Fun realm", "best_for": "Weekend chaos",
      "pitch": "Fast and loud: 10x, a boost to 80, a mount at level 1, gold to spend and events every hour.",
      "set": { "xp": 10, "rested": 4, "drops": 3, "gold": 5, "rep": 5, "honor": 5, "skill": 5, "start_gold": 100, "bots": 60,
        "hubs": true, "free_repair": true } },
    { "id": "rp", "name": "RP realm", "best_for": "Storytellers",
      "pitch": "A normal pace, names that fit the world, no open-world PvP, and gentle evening events.",
      "set": { "boost": false, "mount": false, "xp": 1, "rep": 1, "skill": 1, "bots": 0, "realm_type": "rp", "duel_ladder": false,
        "hide": false, "rp_names": true } },
    { "id": "hardcore", "name": "Hardcore", "best_for": "Challenge seekers",
      "pitch": "1x, no boost, no bots, no scaling. Players pick one life at the Shrine of Challenge.",
      "set": { "boost": false, "mount": false, "autolearn": false, "xp": 1, "rested": 1, "rep": 1, "skill": 1, "bots": 0, "scaling": false,
        "duel_ladder": false, "trivia": false, "hide": false, "one_life": true } },
    { "id": "solo-bots", "name": "Solo with bots", "best_for": "Playing alone",
      "pitch": "Just you, at 2x. Helper bots fill your group and dungeons scale to you. Private only.",
      "set": { "world": "solo", "boost": false, "mount": false, "xp": 2, "bots": 20, "helpers": 4, "scaling": true, "duel_ladder": false,
        "events": false, "trivia": false, "hide": false } },
    { "id": "pvp", "name": "PvP and duel ladder", "best_for": "Fighters",
      "pitch": "Everyone at 80. Open-world PvP, rated duels with weekly points, and 3x honor.",
      "set": { "world": "top", "boost": false, "mount": false, "honor": 3, "bots": 0, "realm_type": "pvp", "duel_reset": true,
        "duel_ladder": true, "trivia": false, "hide": false, "hubs": true, "free_repair": true } },
    { "id": "event-night", "name": "Event night", "best_for": "Game nights",
      "pitch": "A light hangout in Dalaran: hide and seek, trivia, minigolf and friends' game modes.",
      "set": { "world": "hangout", "bots": 0, "duel_ladder": false, "hubs": true, "free_repair": true, "minigolf": true } },
    { "id": "blank", "name": "Blank", "best_for": "Doing it yourself", "blank": true,
      "pitch": "No settings of ours: the Classic levelling ruleset exactly as it ships. Change what you like.",
      "set": {} }
  ]
};
