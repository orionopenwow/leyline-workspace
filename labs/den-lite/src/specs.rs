//! Specs, talents and the action bars (owner 3 Nov: "spellbook placement loadout instant on bars"): every class has
//! its three specs (its three talent trees). Picking a class and a spec fills the spellbook with what that spec has at
//! the level, and every bar at once with a layout a player of that spec would make:
//!
//! * the main bar (buttons 1-12): the rotation on 1-6, area spells and the big cooldowns on 7-12;
//! * the bottom left bar: cooldowns and defensives; the bottom right bar: utility (interrupts, crowd control,
//!   movement, the auto attack); the right bar: mounts; the second right bar: buffs, heals and every other spell the
//!   spec knows that isn't on a bar yet (then the main bar's second page), so every spell is somewhere;
//! * the stance / form bars the game swaps in (a warrior's stances, a druid's cat / prowl / bear / moonkin, a rogue's
//!   stealth, a priest's shadowform): the spec's rotation for that form. The stance bar itself the game builds from
//!   the spellbook; a pet's bar comes with the pet (pets.rs).
//!
//! Talents: with the player's own `Talent.dbc` and `TalentTab.dbc` (copied by the launcher like the other tables),
//! the spec's tree gets its points top to bottom (abilities first, every rank, the tree's last talent at 51 points),
//! the rest go into a second tree, as a level-80 player would build it; the talents' spells are learnt (their
//! passives count in the sandbox: damage, healing and critical bonuses, procs) and the other trees' abilities are left
//! out. The game is told with SMSG_TALENTS_INFO (as AzerothCore's Player::SendTalentsInfoData), so its talent frame
//! shows the build. Without those tables: the spec's bars and every class ability, as before.
//!
//! The button numbers (3.3.5a FrameXML): main bar 0-11 (its second page 12-23), the right bar 24-35, the second right
//! bar 36-47, the bottom right bar 48-59, the bottom left bar 60-71, the bonus (form) bars 72-83, 84-95, 96-107,
//! 108-119. The game shows the extra bars when PLAYER_FIELD_BYTES' third byte has their bits (sandbox.rs sets all
//! four for a class picked).

use crate::spells::{Book, Spell, Table};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

pub const TALENT: &str = "Talent.dbc";
pub const TALENT_TAB: &str = "TalentTab.dbc";
/// The tables this module reads (the launcher copies them: host/lite.rs GAME_TABLES).
pub const FILES: [&str; 2] = [TALENT, TALENT_TAB];
pub const SMSG_TALENTS_INFO: u16 = 0x4C0;

pub const MAIN: usize = 0;
pub const PAGE2: usize = 12;
pub const RIGHT: usize = 24;
pub const RIGHT2: usize = 36;
pub const BOTTOM_RIGHT: usize = 48;
pub const BOTTOM_LEFT: usize = 60;
/// A bonus (form) bar's first button (1-4).
pub fn bonus(n: u8) -> usize {
    72 + (n.clamp(1, 4) as usize - 1) * 12
}
/// PLAYER_FIELD_BYTES' third byte: the four extra bars shown.
pub const ALL_BARS: u8 = 0x0F;

/// The auto attack and the hunter's ranged one (classes.rs has the same numbers).
const ATTACK: u32 = 6603;
const AUTO_SHOT: u32 = 75;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Melee,
    Ranged,
    Caster,
    Healer,
    Tank,
}

impl Role {
    pub fn word(self) -> &'static str {
        match self {
            Role::Melee => "melee",
            Role::Ranged => "ranged",
            Role::Caster => "caster",
            Role::Healer => "healer",
            Role::Tank => "tank",
        }
    }
}

/// A spec: its class, word, name, talent tree (TalentTab's page 0-2), the tree its other points go to, its role,
/// and its bars by spell name (the highest rank known of each; names the spellbook hasn't are skipped).
#[derive(Debug)]
pub struct SpecDef {
    pub class: u8,
    pub word: &'static str,
    pub other: &'static [&'static str],
    pub name: &'static str,
    pub tree: u8,
    pub second: u8,
    pub role: Role,
    pub rotation: &'static [&'static str],
    pub cooldowns: &'static [&'static str],
    pub utility: &'static [&'static str],
    pub support: &'static [&'static str],
    /// Bonus bars (1-4) and their spells (none: the rotation).
    pub forms: &'static [(u8, &'static [&'static str])],
}

const WAR_CD: &[&str] = &["Recklessness", "Bloodrage", "Berserker Rage", "Retaliation", "Shield Wall", "Last Stand", "Enraged Regeneration", "Spell Reflection", "Intimidating Shout", "Shattering Throw", "Heroic Throw", "Shield Block"];
const WAR_UT: &[&str] = &["Attack", "Charge", "Intercept", "Intervene", "Pummel", "Shield Bash", "Hamstring", "Piercing Howl", "Taunt", "Disarm", "Demoralizing Shout", "Victory Rush"];
const WAR_SUP: &[&str] = &["Battle Shout", "Commanding Shout", "Vigilance", "Sunder Armor", "Mocking Blow", "Challenging Shout", "Heroic Fury"];
const PAL_CD: &[&str] = &["Avenging Wrath", "Divine Shield", "Divine Protection", "Lay on Hands", "Hand of Protection", "Hand of Sacrifice", "Divine Sacrifice", "Divine Plea", "Hammer of Justice", "Repentance", "Turn Evil", "Divine Favor"];
const PAL_UT: &[&str] = &["Attack", "Hand of Freedom", "Hand of Salvation", "Cleanse", "Seal of Righteousness", "Seal of Command", "Seal of Light", "Seal of Wisdom", "Seal of Vengeance", "Righteous Fury", "Hand of Reckoning", "Redemption"];
const PAL_SUP: &[&str] = &["Blessing of Kings", "Blessing of Might", "Blessing of Wisdom", "Blessing of Sanctuary", "Greater Blessing of Kings", "Devotion Aura", "Retribution Aura", "Concentration Aura", "Crusader Aura", "Holy Light", "Flash of Light", "Sense Undead"];
const HUN_CD: &[&str] = &["Rapid Fire", "Bestial Wrath", "Readiness", "Deterrence", "Feign Death", "Disengage", "Misdirection", "Master's Call", "Kill Command", "Intimidation", "Silencing Shot", "Scatter Shot"];
const HUN_UT: &[&str] = &["Raptor Strike", "Mongoose Bite", "Wing Clip", "Concussive Shot", "Freezing Trap", "Frost Trap", "Immolation Trap", "Explosive Trap", "Snake Trap", "Tranquilizing Shot", "Distracting Shot", "Flare"];
const HUN_SUP: &[&str] = &["Call Pet", "Dismiss Pet", "Revive Pet", "Mend Pet", "Aspect of the Dragonhawk", "Aspect of the Hawk", "Aspect of the Viper", "Aspect of the Cheetah", "Aspect of the Pack", "Aspect of the Wild", "Trueshot Aura", "Track Humanoids"];
const ROG_CD: &[&str] = &["Vanish", "Evasion", "Cloak of Shadows", "Sprint", "Tricks of the Trade", "Blind", "Kidney Shot", "Gouge", "Dismantle", "Feint", "Preparation", "Cold Blood"];
const ROG_UT: &[&str] = &["Attack", "Stealth", "Kick", "Sap", "Distract", "Pick Pocket", "Throw", "Deadly Throw", "Shoot", "Detect Traps", "Disarm Trap", "Pick Lock"];
const ROG_SUP: &[&str] = &["Expose Armor", "Shiv", "Fan of Knives"];
const PRI_CD: &[&str] = &["Shadowfiend", "Power Infusion", "Pain Suppression", "Guardian Spirit", "Dispersion", "Psychic Scream", "Fade", "Hymn of Hope", "Divine Hymn", "Inner Focus", "Desperate Prayer", "Psychic Horror"];
const PRI_UT: &[&str] = &["Dispel Magic", "Abolish Disease", "Cure Disease", "Mass Dispel", "Mind Control", "Shackle Undead", "Levitate", "Mind Soothe", "Mind Vision", "Shoot", "Resurrection", "Fear Ward"];
const PRI_SUP: &[&str] = &["Power Word: Fortitude", "Prayer of Fortitude", "Divine Spirit", "Shadow Protection", "Inner Fire", "Shadowform", "Vampiric Embrace", "Flash Heal", "Renew", "Power Word: Shield", "Prayer of Healing", "Holy Nova"];
const DK_CD: &[&str] = &["Army of the Dead", "Empower Rune Weapon", "Icebound Fortitude", "Anti-Magic Shell", "Anti-Magic Zone", "Lichborne", "Rune Tap", "Vampiric Blood", "Raise Dead", "Death Pact", "Summon Gargoyle", "Unbreakable Armor"];
const DK_UT: &[&str] = &["Attack", "Death Grip", "Mind Freeze", "Strangulate", "Chains of Ice", "Dark Command", "Death and Decay", "Blood Tap", "Path of Frost", "Raise Ally", "Corpse Explosion", "Hungering Cold"];
const DK_SUP: &[&str] = &["Horn of Winter", "Blood Presence", "Frost Presence", "Unholy Presence", "Bone Shield", "Death Gate"];
const SHA_CD: &[&str] = &["Heroism", "Bloodlust", "Elemental Mastery", "Nature's Swiftness", "Fire Elemental Totem", "Earth Elemental Totem", "Shamanistic Rage", "Feral Spirit", "Grounding Totem", "Tremor Totem", "Mana Tide Totem", "Thunderstorm"];
const SHA_UT: &[&str] = &["Attack", "Wind Shear", "Purge", "Hex", "Frost Shock", "Earthbind Totem", "Cleanse Spirit", "Cure Toxins", "Ghost Wolf", "Water Walking", "Water Breathing", "Ancestral Spirit"];
const SHA_SUP: &[&str] = &["Windfury Weapon", "Flametongue Weapon", "Earthliving Weapon", "Lightning Shield", "Water Shield", "Strength of Earth Totem", "Windfury Totem", "Wrath of Air Totem", "Mana Spring Totem", "Totem of Wrath", "Flametongue Totem", "Healing Wave"];
const MAG_CD: &[&str] = &["Icy Veins", "Mirror Image", "Evocation", "Ice Block", "Invisibility", "Mana Shield", "Ice Barrier", "Cold Snap", "Presence of Mind", "Arcane Power", "Combustion", "Summon Water Elemental"];
const MAG_UT: &[&str] = &["Counterspell", "Spellsteal", "Polymorph", "Blink", "Slow Fall", "Remove Curse", "Frost Nova", "Cone of Cold", "Shoot", "Conjure Refreshment", "Conjure Mana Gem", "Ritual of Refreshment"];
const MAG_SUP: &[&str] = &["Arcane Intellect", "Arcane Brilliance", "Dalaran Intellect", "Molten Armor", "Mage Armor", "Frost Armor", "Ice Armor", "Amplify Magic", "Dampen Magic", "Focus Magic", "Fire Ward", "Frost Ward"];
const LCK_CD: &[&str] = &["Metamorphosis", "Demonic Empowerment", "Shadowfury", "Death Coil", "Howl of Terror", "Fear", "Shadow Ward", "Soulshatter", "Demonic Circle: Teleport", "Demonic Circle: Summon", "Shadowburn", "Curse of Doom"];
const LCK_UT: &[&str] = &["Banish", "Enslave Demon", "Curse of Tongues", "Curse of Weakness", "Curse of Exhaustion", "Create Healthstone", "Create Soulstone", "Ritual of Summoning", "Ritual of Souls", "Shoot", "Eye of Kilrogg", "Unending Breath"];
const LCK_SUP: &[&str] = &["Summon Imp", "Summon Voidwalker", "Summon Succubus", "Summon Felhunter", "Summon Felguard", "Fel Armor", "Demon Armor", "Detect Invisibility", "Health Funnel", "Soul Link", "Dark Pact", "Drain Mana"];
const DRU_CD: &[&str] = &["Innervate", "Barkskin", "Survival Instincts", "Berserk", "Starfall", "Force of Nature", "Nature's Grasp", "Tranquility", "Rebirth", "Nature's Swiftness", "Frenzied Regeneration", "Dash"];
const DRU_UT: &[&str] = &["Attack", "Entangling Roots", "Cyclone", "Hibernate", "Soothe Animal", "Remove Curse", "Abolish Poison", "Cure Poison", "Revive", "Travel Form", "Aquatic Form", "Faerie Fire"];
const DRU_SUP: &[&str] = &["Mark of the Wild", "Gift of the Wild", "Thorns", "Omen of Clarity", "Dire Bear Form", "Bear Form", "Cat Form", "Moonkin Form", "Tree of Life", "Swift Flight Form", "Flight Form", "Rejuvenation"];

/// Every spec. TODO(owner): the layouts are a first pass from the classes' usual builds; say what to change.
pub const SPECS: &[SpecDef] = &[
    // warrior: Arms 0, Fury 1, Protection 2
    SpecDef { class: 1, word: "arms", other: &[], name: "Arms", tree: 0, second: 1, role: Role::Melee,
        rotation: &["Mortal Strike", "Overpower", "Execute", "Rend", "Slam", "Heroic Strike", "Bladestorm", "Sweeping Strikes", "Cleave", "Thunder Clap", "Whirlwind", "Charge"],
        cooldowns: WAR_CD, utility: WAR_UT, support: WAR_SUP, forms: &[(1, &[]), (2, &[]), (3, &[])] },
    SpecDef { class: 1, word: "fury", other: &[], name: "Fury", tree: 1, second: 0, role: Role::Melee,
        rotation: &["Bloodthirst", "Whirlwind", "Slam", "Execute", "Heroic Strike", "Cleave", "Death Wish", "Recklessness", "Berserker Rage", "Heroic Throw", "Intercept", "Victory Rush"],
        cooldowns: WAR_CD, utility: WAR_UT, support: WAR_SUP, forms: &[(1, &[]), (2, &[]), (3, &[])] },
    SpecDef { class: 1, word: "protection", other: &["prot", "tank"], name: "Protection", tree: 2, second: 0, role: Role::Tank,
        rotation: &["Shield Slam", "Revenge", "Devastate", "Thunder Clap", "Shockwave", "Concussion Blow", "Heroic Strike", "Cleave", "Shield Block", "Taunt", "Shield Wall", "Last Stand"],
        cooldowns: WAR_CD, utility: WAR_UT, support: WAR_SUP, forms: &[(1, &[]), (2, &[]), (3, &[])] },
    // paladin: Holy 0, Protection 1, Retribution 2
    SpecDef { class: 2, word: "holy", other: &["healer"], name: "Holy", tree: 0, second: 2, role: Role::Healer,
        rotation: &["Holy Light", "Flash of Light", "Holy Shock", "Beacon of Light", "Sacred Shield", "Judgement of Light", "Lay on Hands", "Divine Illumination", "Divine Favor", "Aura Mastery", "Cleanse", "Divine Plea"],
        cooldowns: PAL_CD, utility: PAL_UT, support: PAL_SUP, forms: &[] },
    SpecDef { class: 2, word: "protection", other: &["prot", "tank"], name: "Protection", tree: 1, second: 2, role: Role::Tank,
        rotation: &["Hammer of the Righteous", "Shield of Righteousness", "Holy Shield", "Consecration", "Judgement of Wisdom", "Avenger's Shield", "Exorcism", "Holy Wrath", "Hand of Reckoning", "Righteous Defense", "Divine Protection", "Divine Plea"],
        cooldowns: PAL_CD, utility: PAL_UT, support: PAL_SUP, forms: &[] },
    SpecDef { class: 2, word: "retribution", other: &["ret"], name: "Retribution", tree: 2, second: 0, role: Role::Melee,
        rotation: &["Crusader Strike", "Judgement of Light", "Divine Storm", "Consecration", "Exorcism", "Hammer of Wrath", "Avenging Wrath", "Holy Wrath", "Seal of Command", "Seal of Righteousness", "Divine Plea", "Hand of Reckoning"],
        cooldowns: PAL_CD, utility: PAL_UT, support: PAL_SUP, forms: &[] },
    // hunter: Beast Mastery 0, Marksmanship 1, Survival 2
    SpecDef { class: 3, word: "beastmastery", other: &["bm", "beast", "beasts"], name: "Beast Mastery", tree: 0, second: 1, role: Role::Ranged,
        rotation: &["Auto Shot", "Steady Shot", "Arcane Shot", "Kill Command", "Serpent Sting", "Multi-Shot", "Kill Shot", "Bestial Wrath", "Intimidation", "Hunter's Mark", "Volley", "Aspect of the Dragonhawk"],
        cooldowns: HUN_CD, utility: HUN_UT, support: HUN_SUP, forms: &[] },
    SpecDef { class: 3, word: "marksmanship", other: &["mm", "marks"], name: "Marksmanship", tree: 1, second: 0, role: Role::Ranged,
        rotation: &["Auto Shot", "Steady Shot", "Chimera Shot", "Aimed Shot", "Arcane Shot", "Serpent Sting", "Kill Shot", "Multi-Shot", "Readiness", "Silencing Shot", "Hunter's Mark", "Volley"],
        cooldowns: HUN_CD, utility: HUN_UT, support: HUN_SUP, forms: &[] },
    SpecDef { class: 3, word: "survival", other: &["sv", "surv"], name: "Survival", tree: 2, second: 1, role: Role::Ranged,
        rotation: &["Auto Shot", "Explosive Shot", "Steady Shot", "Black Arrow", "Serpent Sting", "Aimed Shot", "Kill Shot", "Multi-Shot", "Explosive Trap", "Hunter's Mark", "Volley", "Wyvern Sting"],
        cooldowns: HUN_CD, utility: HUN_UT, support: HUN_SUP, forms: &[] },
    // rogue: Assassination 0, Combat 1, Subtlety 2 (bonus bar 1: stealth)
    SpecDef { class: 4, word: "assassination", other: &["assa", "mutilate"], name: "Assassination", tree: 0, second: 1, role: Role::Melee,
        rotation: &["Mutilate", "Envenom", "Rupture", "Slice and Dice", "Hunger for Blood", "Garrote", "Cold Blood", "Fan of Knives", "Eviscerate", "Expose Armor", "Kick", "Vanish"],
        cooldowns: ROG_CD, utility: ROG_UT, support: ROG_SUP,
        forms: &[(1, &["Garrote", "Ambush", "Cheap Shot", "Sap", "Mutilate", "Envenom", "Rupture", "Slice and Dice", "Pick Pocket", "Distract", "Shadowstep", "Premeditation"])] },
    SpecDef { class: 4, word: "combat", other: &[], name: "Combat", tree: 1, second: 0, role: Role::Melee,
        rotation: &["Sinister Strike", "Slice and Dice", "Rupture", "Eviscerate", "Killing Spree", "Blade Flurry", "Adrenaline Rush", "Fan of Knives", "Expose Armor", "Riposte", "Kick", "Feint"],
        cooldowns: ROG_CD, utility: ROG_UT, support: ROG_SUP,
        forms: &[(1, &["Ambush", "Cheap Shot", "Garrote", "Sap", "Sinister Strike", "Slice and Dice", "Eviscerate", "Rupture", "Pick Pocket", "Distract", "Sprint", "Vanish"])] },
    SpecDef { class: 4, word: "subtlety", other: &["sub"], name: "Subtlety", tree: 2, second: 0, role: Role::Melee,
        rotation: &["Hemorrhage", "Backstab", "Rupture", "Eviscerate", "Slice and Dice", "Shadow Dance", "Shadowstep", "Premeditation", "Preparation", "Fan of Knives", "Kick", "Ambush"],
        cooldowns: ROG_CD, utility: ROG_UT, support: ROG_SUP,
        forms: &[(1, &["Ambush", "Garrote", "Cheap Shot", "Premeditation", "Shadowstep", "Sap", "Hemorrhage", "Eviscerate", "Rupture", "Slice and Dice", "Pick Pocket", "Distract"])] },
    // priest: Discipline 0, Holy 1, Shadow 2 (bonus bar 1: shadowform)
    SpecDef { class: 5, word: "discipline", other: &["disc"], name: "Discipline", tree: 0, second: 1, role: Role::Healer,
        rotation: &["Penance", "Power Word: Shield", "Flash Heal", "Greater Heal", "Prayer of Mending", "Renew", "Pain Suppression", "Power Infusion", "Prayer of Healing", "Smite", "Holy Fire", "Shadow Word: Pain"],
        cooldowns: PRI_CD, utility: PRI_UT, support: PRI_SUP, forms: &[] },
    SpecDef { class: 5, word: "holy", other: &["healer"], name: "Holy", tree: 1, second: 0, role: Role::Healer,
        rotation: &["Flash Heal", "Greater Heal", "Renew", "Circle of Healing", "Prayer of Mending", "Prayer of Healing", "Guardian Spirit", "Binding Heal", "Holy Nova", "Lightwell", "Divine Hymn", "Power Word: Shield"],
        cooldowns: PRI_CD, utility: PRI_UT, support: PRI_SUP, forms: &[] },
    SpecDef { class: 5, word: "shadow", other: &["spriest"], name: "Shadow", tree: 2, second: 0, role: Role::Caster,
        rotation: &["Vampiric Touch", "Shadow Word: Pain", "Mind Blast", "Devouring Plague", "Mind Flay", "Shadow Word: Death", "Mind Sear", "Shadowfiend", "Dispersion", "Vampiric Embrace", "Psychic Horror", "Silence"],
        cooldowns: PRI_CD, utility: PRI_UT, support: PRI_SUP, forms: &[(1, &[])] },
    // death knight: Blood 0, Frost 1, Unholy 2 (presences don't change bars)
    SpecDef { class: 6, word: "blood", other: &["tank"], name: "Blood", tree: 0, second: 2, role: Role::Tank,
        rotation: &["Icy Touch", "Plague Strike", "Heart Strike", "Death Strike", "Death Coil", "Blood Boil", "Rune Strike", "Pestilence", "Dancing Rune Weapon", "Hysteria", "Vampiric Blood", "Mark of Blood"],
        cooldowns: DK_CD, utility: DK_UT, support: DK_SUP, forms: &[] },
    SpecDef { class: 6, word: "frost", other: &[], name: "Frost", tree: 1, second: 2, role: Role::Melee,
        rotation: &["Icy Touch", "Plague Strike", "Obliterate", "Frost Strike", "Howling Blast", "Blood Strike", "Death Coil", "Pestilence", "Unbreakable Armor", "Hungering Cold", "Deathchill", "Empower Rune Weapon"],
        cooldowns: DK_CD, utility: DK_UT, support: DK_SUP, forms: &[] },
    SpecDef { class: 6, word: "unholy", other: &[], name: "Unholy", tree: 2, second: 0, role: Role::Melee,
        rotation: &["Icy Touch", "Plague Strike", "Scourge Strike", "Blood Strike", "Death Coil", "Death and Decay", "Pestilence", "Summon Gargoyle", "Bone Shield", "Raise Dead", "Ghoul Frenzy", "Empower Rune Weapon"],
        cooldowns: DK_CD, utility: DK_UT, support: DK_SUP, forms: &[] },
    // shaman: Elemental 0, Enhancement 1, Restoration 2
    SpecDef { class: 7, word: "elemental", other: &["ele"], name: "Elemental", tree: 0, second: 1, role: Role::Caster,
        rotation: &["Flame Shock", "Lava Burst", "Lightning Bolt", "Chain Lightning", "Earth Shock", "Thunderstorm", "Elemental Mastery", "Fire Nova", "Magma Totem", "Searing Totem", "Wind Shear", "Fire Elemental Totem"],
        cooldowns: SHA_CD, utility: SHA_UT, support: SHA_SUP, forms: &[] },
    SpecDef { class: 7, word: "enhancement", other: &["enh", "enhance"], name: "Enhancement", tree: 1, second: 0, role: Role::Melee,
        rotation: &["Stormstrike", "Lava Lash", "Earth Shock", "Flame Shock", "Lightning Bolt", "Shamanistic Rage", "Feral Spirit", "Fire Nova", "Magma Totem", "Chain Lightning", "Wind Shear", "Lightning Shield"],
        cooldowns: SHA_CD, utility: SHA_UT, support: SHA_SUP, forms: &[] },
    SpecDef { class: 7, word: "restoration", other: &["resto", "healer"], name: "Restoration", tree: 2, second: 1, role: Role::Healer,
        rotation: &["Riptide", "Healing Wave", "Lesser Healing Wave", "Chain Heal", "Earth Shield", "Healing Stream Totem", "Nature's Swiftness", "Mana Tide Totem", "Cleanse Spirit", "Tidal Force", "Water Shield", "Earthliving Weapon"],
        cooldowns: SHA_CD, utility: SHA_UT, support: SHA_SUP, forms: &[] },
    // mage: Arcane 0, Fire 1, Frost 2
    SpecDef { class: 8, word: "arcane", other: &[], name: "Arcane", tree: 0, second: 2, role: Role::Caster,
        rotation: &["Arcane Blast", "Arcane Missiles", "Arcane Barrage", "Arcane Power", "Presence of Mind", "Arcane Explosion", "Evocation", "Mirror Image", "Icy Veins", "Flamestrike", "Counterspell", "Fire Blast"],
        cooldowns: MAG_CD, utility: MAG_UT, support: MAG_SUP, forms: &[] },
    SpecDef { class: 8, word: "fire", other: &[], name: "Fire", tree: 1, second: 0, role: Role::Caster,
        rotation: &["Fireball", "Living Bomb", "Pyroblast", "Fire Blast", "Scorch", "Combustion", "Flamestrike", "Dragon's Breath", "Blast Wave", "Mirror Image", "Frostfire Bolt", "Counterspell"],
        cooldowns: MAG_CD, utility: MAG_UT, support: MAG_SUP, forms: &[] },
    SpecDef { class: 8, word: "frost", other: &[], name: "Frost", tree: 2, second: 0, role: Role::Caster,
        rotation: &["Frostbolt", "Ice Lance", "Deep Freeze", "Frostfire Bolt", "Icy Veins", "Cold Snap", "Summon Water Elemental", "Frost Nova", "Cone of Cold", "Ice Barrier", "Mirror Image", "Counterspell"],
        cooldowns: MAG_CD, utility: MAG_UT, support: MAG_SUP, forms: &[] },
    // warlock: Affliction 0, Demonology 1, Destruction 2
    SpecDef { class: 9, word: "affliction", other: &["aff", "affli"], name: "Affliction", tree: 0, second: 2, role: Role::Caster,
        rotation: &["Shadow Bolt", "Haunt", "Corruption", "Unstable Affliction", "Curse of Agony", "Drain Soul", "Life Tap", "Seed of Corruption", "Curse of the Elements", "Death Coil", "Fear", "Summon Felhunter"],
        cooldowns: LCK_CD, utility: LCK_UT, support: LCK_SUP, forms: &[] },
    SpecDef { class: 9, word: "demonology", other: &["demo"], name: "Demonology", tree: 1, second: 2, role: Role::Caster,
        rotation: &["Shadow Bolt", "Incinerate", "Immolate", "Corruption", "Curse of Doom", "Soul Fire", "Metamorphosis", "Immolation Aura", "Demonic Empowerment", "Life Tap", "Seed of Corruption", "Summon Felguard"],
        cooldowns: LCK_CD, utility: LCK_UT, support: LCK_SUP, forms: &[] },
    SpecDef { class: 9, word: "destruction", other: &["destro"], name: "Destruction", tree: 2, second: 1, role: Role::Caster,
        rotation: &["Immolate", "Conflagrate", "Incinerate", "Chaos Bolt", "Corruption", "Curse of Doom", "Soul Fire", "Shadowfury", "Shadowburn", "Rain of Fire", "Life Tap", "Summon Imp"],
        cooldowns: LCK_CD, utility: LCK_UT, support: LCK_SUP, forms: &[] },
    // druid: Balance 0, Feral Combat 1, Restoration 2 (bonus bars: 1 cat, 2 prowl, 3 bear, 4 moonkin)
    SpecDef { class: 11, word: "balance", other: &["boomkin", "moonkin"], name: "Balance", tree: 0, second: 2, role: Role::Caster,
        rotation: &["Wrath", "Starfire", "Moonfire", "Insect Swarm", "Starfall", "Faerie Fire", "Typhoon", "Force of Nature", "Hurricane", "Innervate", "Moonkin Form", "Barkskin"],
        cooldowns: DRU_CD, utility: DRU_UT, support: DRU_SUP, forms: &[(4, &[])] },
    SpecDef { class: 11, word: "feral", other: &["cat", "bear", "tank"], name: "Feral", tree: 1, second: 2, role: Role::Melee,
        rotation: &["Mangle (Cat)", "Shred", "Rake", "Rip", "Savage Roar", "Ferocious Bite", "Tiger's Fury", "Berserk", "Faerie Fire (Feral)", "Swipe (Cat)", "Maim", "Cat Form"],
        cooldowns: DRU_CD, utility: DRU_UT, support: DRU_SUP,
        forms: &[
            (1, &[]),
            (2, &["Ravage", "Pounce", "Shred", "Rake", "Mangle (Cat)", "Dash", "Tiger's Fury", "Savage Roar", "Rip", "Ferocious Bite", "Faerie Fire (Feral)", "Cat Form"]),
            (3, &["Mangle (Bear)", "Lacerate", "Maul", "Swipe (Bear)", "Faerie Fire (Feral)", "Demoralizing Roar", "Growl", "Bash", "Feral Charge - Bear", "Survival Instincts", "Frenzied Regeneration", "Enrage"]),
        ] },
    SpecDef { class: 11, word: "restoration", other: &["resto", "tree", "healer"], name: "Restoration", tree: 2, second: 0, role: Role::Healer,
        rotation: &["Rejuvenation", "Lifebloom", "Regrowth", "Wild Growth", "Swiftmend", "Nourish", "Healing Touch", "Tranquility", "Nature's Swiftness", "Innervate", "Tree of Life", "Remove Curse"],
        cooldowns: DRU_CD, utility: DRU_UT, support: DRU_SUP, forms: &[] },
];

/// The default spec of each class (the one `.den class` picks when none is chosen yet).
pub const DEFAULTS: &[(u8, &str)] = &[(1, "arms"), (2, "retribution"), (3, "marksmanship"), (4, "combat"), (5, "shadow"), (6, "frost"), (7, "elemental"), (8, "fire"), (9, "affliction"), (11, "balance")];

pub fn of_class(class: u8) -> Vec<&'static SpecDef> {
    SPECS.iter().filter(|s| s.class == class).collect()
}

pub fn default_of(class: u8) -> Option<&'static SpecDef> {
    let w = DEFAULTS.iter().find(|d| d.0 == class).map(|d| d.1)?;
    SPECS.iter().find(|s| s.class == class && s.word == w)
}

/// A spec of a class by its word, name or a short form ("prot", "ret", "bm", "resto"...).
pub fn by_word(class: u8, w: &str) -> Option<&'static SpecDef> {
    let w: String = w.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if w.is_empty() {
        return None;
    }
    of_class(class)
        .into_iter()
        .find(|s| s.word == w || s.other.contains(&w.as_str()) || s.name.to_lowercase().replace(' ', "") == w)
}

// ---------------------------------------------------------------- talents

/// One Talent.dbc row (3.3.5a, 23 fields: id, tab, row, column, five rank spells, ..., depends on (13), its rank (16)).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Talent {
    pub id: u32,
    pub tab: u32,
    pub row: u32,
    pub col: u32,
    pub ranks: [u32; 5],
    pub depends: u32,
    pub depends_rank: u32,
}

impl Talent {
    pub fn max_rank(&self) -> u8 {
        self.ranks.iter().take_while(|r| **r != 0).count() as u8
    }
}

/// One TalentTab.dbc row (24 fields: id, names, ..., class mask (20), pet mask (21), page (22)).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Tab {
    pub id: u32,
    pub class_mask: u32,
    pub page: u32,
}

/// The talent tables (empty when the player's own aren't in the Dens folder).
#[derive(Debug, Default, Clone)]
pub struct Talents {
    pub talents: Vec<Talent>,
    pub tabs: Vec<Tab>,
}

/// A build: (talent id, points in it), in the order spent.
pub type Build = Vec<(u32, u8)>;

impl Talents {
    pub fn from_bytes(talent: Option<&[u8]>, tab: Option<&[u8]>) -> Talents {
        let mut out = Talents::default();
        if let Some(t) = talent.and_then(Table::parse).filter(|t| t.fields == 23 && t.size == 92) {
            for k in 0..t.n {
                let r = t.row(k);
                let u = |f| Table::u(r, f);
                out.talents.push(Talent { id: u(0), tab: u(1), row: u(2), col: u(3), ranks: [u(4), u(5), u(6), u(7), u(8)], depends: u(13), depends_rank: u(16) });
            }
        }
        if let Some(t) = tab.and_then(Table::parse).filter(|t| t.fields == 24 && t.size == 96) {
            for k in 0..t.n {
                let r = t.row(k);
                out.tabs.push(Tab { id: Table::u(r, 0), class_mask: Table::u(r, 20), page: Table::u(r, 22) });
            }
        }
        out
    }

    pub fn read(dir: &Path) -> Talents {
        let a = std::fs::read(dir.join(TALENT)).ok();
        let b = std::fs::read(dir.join(TALENT_TAB)).ok();
        Talents::from_bytes(a.as_deref(), b.as_deref())
    }

    pub fn is_empty(&self) -> bool {
        self.talents.is_empty() || self.tabs.is_empty()
    }

    /// A class's tab on a page (0-2).
    pub fn tab(&self, class: u8, page: u8) -> Option<u32> {
        let bit = 1u32 << (class.max(1) - 1);
        self.tabs.iter().find(|t| t.class_mask & bit != 0 && t.page == page as u32).map(|t| t.id)
    }

    fn by_id(&self, id: u32) -> Option<&Talent> {
        self.talents.iter().find(|t| t.id == id)
    }

    /// The class's talents (all three trees).
    pub fn of_class(&self, class: u8) -> Vec<&Talent> {
        let tabs: Vec<u32> = (0..3).filter_map(|p| self.tab(class, p)).collect();
        self.talents.iter().filter(|t| tabs.contains(&t.tab)).collect()
    }

    /// The spec's talents at a level (a point a level from 10: 71 at 80): the spec's tree first, top to bottom,
    /// abilities before passives, every rank, the tree's deepest talent (and what it needs) kept for its turn; up to
    /// 51 there, the rest into the second tree the same way. Only builds the game allows (5 points a row, what a
    /// talent depends on first).
    pub fn build(&self, book: &Book, spec: &SpecDef, level: u32) -> Build {
        let mut left = level.saturating_sub(9).min(71) as i32;
        let mut out: Build = Vec::new();
        for (page, cap) in [(spec.tree, 51), (spec.second, 71)] {
            if left <= 0 {
                break;
            }
            let Some(tab) = self.tab(spec.class, page) else { continue };
            let spend = left.min(cap);
            let got = self.fill(book, tab, spend);
            left -= got.iter().map(|x| x.1 as i32).sum::<i32>();
            out.extend(got);
        }
        out
    }

    /// Up to `budget` points in one tree.
    fn fill(&self, book: &Book, tab: u32, budget: i32) -> Build {
        let mut tree: Vec<&Talent> = self.talents.iter().filter(|t| t.tab == tab && t.max_rank() > 0).collect();
        tree.sort_by_key(|t| (t.row, t.col, t.id));
        let Some(deepest_row) = tree.iter().map(|t| t.row).max() else { return Vec::new() };
        // the deepest talent and the talents it needs: kept for when their rows come
        let mut must: HashSet<u32> = HashSet::new();
        let reaches = budget > 5 * deepest_row as i32;
        if let Some(d) = tree.iter().filter(|t| t.row == deepest_row && reaches).min_by_key(|t| t.col) {
            let mut at = Some(**d);
            while let Some(t) = at {
                if !must.insert(t.id) {
                    break;
                }
                at = if t.depends != 0 { self.by_id(t.depends).copied() } else { None };
            }
        }
        let active = |t: &Talent| book.spell(t.ranks[0]).is_some_and(|s| !s.passive());
        let mut spent: BTreeMap<u32, u8> = BTreeMap::new();
        let mut order: Build = Vec::new();
        let mut total = 0i32;
        for row in 0..=deepest_row {
            if total < 5 * row as i32 {
                break; // this row isn't open (not enough points above it)
            }
            let mut here: Vec<&Talent> = tree.iter().copied().filter(|t| t.row == row).collect();
            // what must come first, then abilities, then passives
            here.sort_by_key(|t| (!must.contains(&t.id), !active(t), t.col));
            for t in here {
                let need_ok = t.depends == 0 || spent.get(&t.depends).copied().unwrap_or(0) as u32 > t.depends_rank;
                if !need_ok {
                    continue;
                }
                // points kept back for the must-haves further down
                let reserve: i32 = must
                    .iter()
                    .filter(|id| **id != t.id && !spent.contains_key(id))
                    .filter_map(|id| self.by_id(*id))
                    .filter(|m| m.row > row)
                    .map(|m| if m.row == deepest_row { 1 } else { m.max_rank() as i32 })
                    .sum();
                // the deepest row needs 5 per row above it: never leave the tree short of that
                let room = budget - total - if must.contains(&t.id) { 0 } else { reserve };
                if room <= 0 {
                    continue;
                }
                let n = (t.max_rank() as i32).min(room) as u8;
                if t.row == deepest_row && total < 5 * deepest_row as i32 {
                    continue;
                }
                spent.insert(t.id, n);
                order.push((t.id, n));
                total += n as i32;
                if total >= budget {
                    return order;
                }
            }
        }
        order
    }

    /// The spells a build gives (each talent's spell at its rank) and the spells of the class's other talents (by
    /// name: their trainer ranks too), which a spec without them doesn't have.
    pub fn spells(&self, book: &Book, class: u8, build: &Build) -> (Vec<u32>, HashSet<String>) {
        let chosen: HashMap<u32, u8> = build.iter().copied().collect();
        let mut give = Vec::new();
        let mut names_in: HashSet<String> = HashSet::new();
        let mut names_out: HashSet<String> = HashSet::new();
        for t in self.of_class(class) {
            let name = book.spell(t.ranks[0]).map(|s| s.name.clone()).unwrap_or_default();
            match chosen.get(&t.id) {
                Some(&n) if n > 0 => {
                    let id = t.ranks[(n as usize - 1).min(4)];
                    if id != 0 {
                        give.push(id);
                    }
                    names_in.insert(name);
                }
                _ => {
                    names_out.insert(name);
                }
            }
        }
        names_out.retain(|n| !n.is_empty() && !names_in.contains(n));
        give.sort_unstable();
        (give, names_out)
    }
}

/// The spec's spellbook: the class's spells at the level without the other specs' talent abilities, with the build's
/// talents (sorted, no doubles). No talent tables: the class's spells as they are.
pub fn spec_spells(book: &Book, talents: &Talents, class_spells: &[u32], class: u8, build: &Build) -> Vec<u32> {
    if talents.is_empty() {
        return class_spells.to_vec();
    }
    let (give, out) = talents.spells(book, class, build);
    let mut v: Vec<u32> = class_spells.iter().copied().filter(|id| !book.spell(*id).is_some_and(|s| out.contains(&s.name))).collect();
    v.extend(give);
    v.sort_unstable();
    v.dedup();
    v
}

/// SMSG_TALENTS_INFO for a player: u8 0, u32 points left, u8 specs (1), u8 active (0), then the spec: u8 talents,
/// each u32 talent and u8 rank (0-based), u8 glyphs (6) and six u16 0.
pub fn talents_info_body(build: &Build, points_left: u32) -> Vec<u8> {
    let n = build.iter().filter(|x| x.1 > 0).count().min(255);
    let mut b = Vec::with_capacity(9 + n * 5 + 13);
    b.push(0);
    b.extend_from_slice(&points_left.to_le_bytes());
    b.push(1);
    b.push(0);
    b.push(n as u8);
    for &(id, r) in build.iter().filter(|x| x.1 > 0).take(255) {
        b.extend_from_slice(&id.to_le_bytes());
        b.push(r - 1);
    }
    b.push(6);
    b.extend_from_slice(&[0u8; 12]);
    b
}

// ---------------------------------------------------------------- the bars

/// The spell known by this name (the highest level, not passive); the auto attacks by their names.
fn by_name(book: &Book, known: &[u32], name: &str) -> Option<u32> {
    if name == "Attack" {
        return known.contains(&ATTACK).then_some(ATTACK);
    }
    if name == "Auto Shot" {
        return known.contains(&AUTO_SHOT).then_some(AUTO_SHOT);
    }
    known.iter().filter_map(|id| book.spell(*id)).filter(|s| s.name == name && !s.passive()).max_by_key(|s| (s.spell_level, s.id)).map(|s| s.id)
}

fn hits_or_heals(s: &Spell) -> bool {
    s.effects.iter().any(|e| matches!(e.kind, 2 | 9 | 10 | 17 | 31 | 58 | 121 | 136) || (e.kind == 6 && matches!(e.aura, 3 | 8 | 23 | 53)))
}

fn is_mount(s: &Spell) -> bool {
    s.effects.iter().any(|e| e.kind == 6 && e.aura == 78)
}

/// Every button (144, packed: a spell id, type 0) for a spec: see the module's notes. `mounts`: the mounts known
/// (on the right bar, after the class's own).
pub fn layout(book: &Book, spec: &SpecDef, known: &[u32], mounts: &[u32]) -> Vec<u32> {
    let mut b = vec![0u32; 144];
    let mut used: HashSet<u32> = HashSet::new();
    let place = |b: &mut Vec<u32>, used: &mut HashSet<u32>, start: usize, names: &[&str], dedupe: bool| -> usize {
        let mut at = start;
        for n in names {
            if at >= start + 12 {
                break;
            }
            if let Some(id) = by_name(book, known, n) {
                if dedupe && used.contains(&id) {
                    continue;
                }
                if b[start..at].contains(&id) {
                    continue;
                }
                b[at] = id;
                if dedupe {
                    used.insert(id);
                }
                at += 1;
            }
        }
        at
    };
    // the main bar: the rotation, then the spec's other damage or healing
    let mut at = place(&mut b, &mut used, MAIN, spec.rotation, true);
    for id in known {
        if at >= MAIN + 12 {
            break;
        }
        if used.contains(id) {
            continue;
        }
        if let Some(s) = book.spell(*id) {
            if !s.passive() && hits_or_heals(s) && !is_mount(s) && !b[MAIN..at].iter().any(|x| book.spell(*x).is_some_and(|y| y.name == s.name)) {
                b[at] = *id;
                used.insert(*id);
                at += 1;
            }
        }
    }
    // the hunter's Auto Shot: always on the main bar
    if known.contains(&AUTO_SHOT) && !b[MAIN..MAIN + 12].contains(&AUTO_SHOT) {
        b[MAIN + 11] = AUTO_SHOT;
        used.insert(AUTO_SHOT);
    }
    place(&mut b, &mut used, BOTTOM_LEFT, spec.cooldowns, true);
    place(&mut b, &mut used, BOTTOM_RIGHT, spec.utility, true);
    // the right bar: mounts (the class's own first)
    let mut mine: Vec<u32> = known.iter().copied().filter(|id| book.spell(*id).is_some_and(|s| is_mount(s) && !s.passive())).collect();
    for m in mounts {
        if !mine.contains(m) {
            mine.push(*m);
        }
    }
    for (k, id) in mine.iter().take(12).enumerate() {
        b[RIGHT + k] = *id;
        used.insert(*id);
    }
    // the second right bar: buffs and the rest, then the main bar's second page
    let at = place(&mut b, &mut used, RIGHT2, spec.support, true);
    let mut slots: Vec<usize> = (at..RIGHT2 + 12).collect();
    slots.extend(PAGE2..PAGE2 + 12);
    let mut free = slots.into_iter();
    for id in known {
        if used.contains(id) || *id == ATTACK {
            continue;
        }
        let Some(s) = book.spell(*id) else { continue };
        if s.passive() || is_mount(s) || s.name.is_empty() {
            continue;
        }
        // one rank of a name only
        if b.iter().any(|x| *x != 0 && book.spell(*x).is_some_and(|y| y.name == s.name)) {
            continue;
        }
        match free.next() {
            Some(slot) => {
                b[slot] = *id;
                used.insert(*id);
            }
            None => break,
        }
    }
    // the form bars: their own list, or the main bar again
    for &(n, names) in spec.forms {
        let start = bonus(n);
        if names.is_empty() {
            let main: Vec<u32> = b[MAIN..MAIN + 12].to_vec();
            b[start..start + 12].copy_from_slice(&main);
        } else {
            let mut ignore = HashSet::new();
            place(&mut b, &mut ignore, start, names, false);
        }
    }
    b
}

/// The passives' effects on the sandbox's numbers (classes::Mods): damage done per cent by school (aura 79,
/// SPELL_AURA_MOD_DAMAGE_PERCENT_DONE), healing done per cent (136), critical chance (57 spells, 52 weapons, 71 a
/// school), and procs (42, PROC_TRIGGER_SPELL: (the passive, the spell it fires, chance per cent)).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Passives {
    pub dmg_pct: f32,
    pub heal_pct: f32,
    pub crit: f32,
    pub procs: Vec<(u32, u32, u32)>,
}

pub fn passives(book: &Book, spells: &[u32]) -> Passives {
    let mut p = Passives::default();
    for id in spells {
        let Some(s) = book.spell(*id) else { continue };
        if !s.passive() {
            continue;
        }
        for e in &s.effects {
            if e.kind != 6 {
                continue;
            }
            let v = (e.base + 1) as f32;
            match e.aura {
                79 if (1..=50).contains(&(e.base + 1)) => p.dmg_pct += v,
                136 if (1..=50).contains(&(e.base + 1)) => p.heal_pct += v,
                52 | 57 | 71 if (1..=25).contains(&(e.base + 1)) => p.crit += v,
                42 if e.trigger != 0 && s.proc_chance > 0 && s.proc_chance <= 100 => p.procs.push((s.id, e.trigger, s.proc_chance)),
                _ => {}
            }
        }
    }
    p.dmg_pct = p.dmg_pct.min(60.0);
    p.heal_pct = p.heal_pct.min(60.0);
    p.crit = p.crit.min(30.0);
    p
}

/// A test world for the talents: two mage trees (Arcane 1, Fire 41) of made-up talents.
pub mod fake {
    use crate::spells::fake::table;

    /// Talent.dbc from (id, tab, row, col, ranks, depends on, its rank).
    pub fn talents(rows: &[(u32, u32, u32, u32, &[u32], u32, u32)]) -> Vec<u8> {
        let v: Vec<Vec<u32>> = rows
            .iter()
            .map(|&(id, tab, row, col, ranks, dep, dep_rank)| {
                let mut r = vec![0u32; 23];
                r[0] = id;
                r[1] = tab;
                r[2] = row;
                r[3] = col;
                for (k, s) in ranks.iter().take(5).enumerate() {
                    r[4 + k] = *s;
                }
                r[13] = dep;
                r[16] = dep_rank;
                r
            })
            .collect();
        table(23, &v, &[])
    }

    /// TalentTab.dbc from (id, class mask, page).
    pub fn tabs(rows: &[(u32, u32, u32)]) -> Vec<u8> {
        let v: Vec<Vec<u32>> = rows
            .iter()
            .map(|&(id, mask, page)| {
                let mut r = vec![0u32; 24];
                r[0] = id;
                r[20] = mask;
                r[22] = page;
                r
            })
            .collect();
        table(24, &v, &[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spells::fake as sf;

    fn mage_specs() -> Vec<&'static SpecDef> {
        of_class(8)
    }

    #[test]
    fn every_class_has_three_specs_with_words_and_bars() {
        for c in [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 11] {
            let s = of_class(c);
            assert_eq!(s.len(), 3, "class {c}");
            let mut trees: Vec<u8> = s.iter().map(|x| x.tree).collect();
            trees.sort_unstable();
            assert_eq!(trees, vec![0, 1, 2], "class {c}: one spec a tree");
            for x in &s {
                assert!(x.second != x.tree && x.second < 3, "{}", x.word);
                assert!(x.rotation.len() >= 6 && x.rotation.len() <= 12, "{}", x.word);
                assert_eq!(by_word(c, x.word).map(|y| y.tree), Some(x.tree));
                for n in x.rotation.iter().chain(x.cooldowns).chain(x.utility).chain(x.support) {
                    assert!(!crate::companion::branded(n), "{n}");
                }
            }
            assert!(default_of(c).is_some(), "class {c}");
        }
        assert_eq!(by_word(1, "prot").map(|s| s.word), Some("protection"));
        assert_eq!(by_word(2, "Ret").map(|s| s.word), Some("retribution"));
        assert_eq!(by_word(3, "BM").map(|s| s.word), Some("beastmastery"));
        assert_eq!(by_word(11, "resto").map(|s| s.tree), Some(2));
        assert_eq!(by_word(11, "Feral").map(|s| s.role), Some(Role::Melee));
        assert!(by_word(8, "holy").is_none() && by_word(8, "").is_none());
        assert_eq!(bonus(1), 72);
        assert_eq!(bonus(4), 108);
    }

    #[test]
    fn the_bars_put_the_rotation_first_and_every_spell_somewhere() {
        let b = sf::book();
        let fire = by_word(8, "fire").unwrap();
        let known = vec![133, 143, 1459, 2136, 5143, 6603, 458];
        let bars = layout(&b, fire, &known, &[]);
        assert_eq!(bars.len(), 144);
        assert_eq!(bars[0], 143, "Fireball first, its highest rank");
        assert_eq!(bars[1], 2136, "Fire Blast next (Living Bomb, Pyroblast unknown)");
        assert!(bars[MAIN..MAIN + 12].contains(&5143), "a damage spell not in the list still on the main bar");
        assert_eq!(bars[RIGHT2], 1459, "Arcane Intellect: a buff, on the second right bar");
        assert_eq!(bars[RIGHT], 458, "the mount on the right bar");
        assert!(!bars.contains(&133), "one rank only");
        // a mount from .den mounts too, after the class's own
        let bars = layout(&b, fire, &known, &[458, 999]);
        assert_eq!(&bars[RIGHT..RIGHT + 2], &[458, 999]);
        // frost: the same spells, its own order
        let frost = by_word(8, "frost").unwrap();
        let fb = layout(&b, frost, &known, &[]);
        assert_ne!(fb[0], 0);
        let _ = mage_specs();
    }

    #[test]
    fn form_bars_take_the_rotation_or_their_own_list() {
        let b = sf::book();
        let arms = by_word(1, "arms").unwrap();
        let known = vec![78, 6603];
        let bars = layout(&b, arms, &known, &[]);
        assert_eq!(bars[MAIN], 78, "Heroic Strike (the only strike known)");
        assert_eq!(bars[bonus(1)], 78, "battle stance's bar: the same");
        assert_eq!(bars[bonus(3)], 78, "berserker stance's bar: the same");
        assert_eq!(bars[BOTTOM_RIGHT], 6603, "the auto attack with the utility");
        let hunter = by_word(3, "mm").unwrap();
        let hb = layout(&b, hunter, &[75, 6603], &[]);
        assert_eq!(hb[MAIN], 75, "Auto Shot");
    }

    fn talent_world() -> (crate::spells::Book, Talents) {
        // spells: a passive 3 % crit (row 0, 5 ranks), an ability "Blast Wave" (row 2), the capstone "Living Bomb"
        let mut rows = sf::sample();
        for (id, name, passive) in [(9001, "Incineration", true), (9002, "Incineration", true), (9003, "Incineration", true), (9100, "Blast Wave", false), (9200, "Living Bomb", false), (9300, "Arcane Focus", true), (9301, "Arcane Focus", true), (9400, "Torment", true)] {
            rows.push(sf::Row { id, name: name.into(), level: 10, attributes: if passive { 0x40 } else { 0 }, cast_index: 1, effects: vec![(6, 1, 1, 1, 57, 0, 0, 0)], ..Default::default() });
        }
        let book = crate::spells::Book::from_bytes(Some(&sf::spells(&rows)), Some(&sf::abilities(&sf::ABILITIES)), Some(&sf::four(&sf::CAST_TIMES)), None, None);
        let mut t: Vec<(u32, u32, u32, u32, &[u32], u32, u32)> = vec![
            (1, 41, 0, 0, &[9001, 9002, 9003], 0, 0),
            (2, 41, 2, 1, &[9100], 0, 0),
            (3, 41, 10, 1, &[9200], 2, 0),
            (10, 61, 0, 0, &[9300, 9301], 0, 0),
            (20, 81, 0, 0, &[9400], 0, 0),
        ];
        // filler passives: 5 ranks on every row of the fire tree (so the rows open)
        let fill: Vec<[u32; 5]> = (0..10u32).map(|r| [7000 + r * 10, 7001 + r * 10, 7002 + r * 10, 7003 + r * 10, 7004 + r * 10]).collect();
        let fill: &'static Vec<[u32; 5]> = Box::leak(Box::new(fill));
        for (r, ranks) in fill.iter().enumerate() {
            t.push((100 + r as u32, 41, r as u32, 2, &ranks[..], 0, 0));
        }
        let talents = Talents::from_bytes(Some(&fake::talents(&t)), Some(&fake::tabs(&[(61, 128, 0), (41, 128, 1), (81, 256, 0)])));
        (book, talents)
    }

    #[test]
    fn a_level_80_build_reaches_the_trees_last_talent_and_puts_the_rest_in_the_second() {
        let (book, t) = talent_world();
        assert_eq!(t.talents.len(), 15);
        assert_eq!(t.tab(8, 1), Some(41));
        assert_eq!(t.tab(8, 0), Some(61));
        assert_eq!(t.tab(9, 0), Some(81), "the warlock's tree isn't the mage's");
        let fire = by_word(8, "fire").unwrap();
        let build = t.build(&book, fire, 80);
        let total: u32 = build.iter().map(|x| x.1 as u32).sum();
        let in_fire: u32 = build.iter().filter(|x| t.by_id(x.0).unwrap().tab == 41).map(|x| x.1 as u32).sum();
        assert!(build.iter().any(|x| x.0 == 3), "the capstone: {build:?}");
        assert!(build.iter().any(|x| x.0 == 2), "the ability it depends on");
        assert!(in_fire <= 51, "{in_fire}");
        assert!(build.iter().any(|x| x.0 == 10 && x.1 == 2), "the second tree (arcane) gets the rest");
        assert!(total <= 71);
        // every talent's row open when it was taken (5 points a row above it)
        let mut so_far: HashMap<u32, u32> = HashMap::new();
        for &(id, n) in &build {
            let tl = t.by_id(id).unwrap();
            assert!(*so_far.get(&tl.tab).unwrap_or(&0) >= tl.row * 5, "{id} at row {}", tl.row);
            *so_far.entry(tl.tab).or_default() += n as u32;
        }
        // a low level: few points, no capstone
        let low = t.build(&book, fire, 15);
        assert_eq!(low.iter().map(|x| x.1 as u32).sum::<u32>(), 6);
        assert!(!low.iter().any(|x| x.0 == 3));
        assert!(t.build(&book, fire, 9).is_empty());
    }

    #[test]
    fn the_specs_spells_and_the_talent_frames_message() {
        let (book, t) = talent_world();
        let fire = by_word(8, "fire").unwrap();
        let build = t.build(&book, fire, 80);
        let class_spells = vec![133, 143, 9100, 9200, 9300];
        let spells = spec_spells(&book, &t, &class_spells, 8, &build);
        assert!(spells.contains(&9200) && spells.contains(&9100) && spells.contains(&9003), "{spells:?}");
        // arcane's talent ability isn't fire's (here it's a passive taken in the second tree: kept)
        let arcane = by_word(8, "arcane").unwrap();
        let ab = t.build(&book, arcane, 80);
        let aspells = spec_spells(&book, &t, &class_spells, 8, &ab);
        assert!(!aspells.contains(&9200) && !aspells.contains(&9100), "fire's talents aren't arcane's: {aspells:?}");
        // without the tables: every spell as it was
        assert_eq!(spec_spells(&book, &Talents::default(), &class_spells, 8, &build), class_spells);
        let body = talents_info_body(&vec![(1, 3), (2, 1)], 0);
        assert_eq!(&body[..8], &[0, 0, 0, 0, 0, 1, 0, 2]);
        assert_eq!(&body[8..13], &[1, 0, 0, 0, 2], "talent 1 at its third rank (0-based 2)");
        assert_eq!(&body[13..18], &[2, 0, 0, 0, 0]);
        assert_eq!(body[18], 6);
        assert_eq!(body.len(), 19 + 12);
        let p = passives(&book, &spells);
        assert!(p.crit >= 2.0, "the passives' critical chance: {p:?}");
    }

    #[test]
    fn a_table_of_another_shape_gives_no_talents() {
        let bad = sf::table(22, &[vec![1, 41]], &[]);
        assert!(Talents::from_bytes(Some(&bad), Some(&fake::tabs(&[(41, 128, 1)]))).is_empty());
    }
}
