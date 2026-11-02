//! Leyline's core: game clients, launching, settings, the mod catalog and mod manager, the realm list,
//! hosting through the host kit, and the launcher's own update check.
//!
//! Everything here is plain Rust with no webview, so `cargo test --no-default-features` runs it anywhere.
//! The desktop app (`app` feature) wraps these modules in Tauri commands; see `app.rs`.
//!
//! Leyline never offers game files: a client is the player's own folder, zip or link to their own backup
//! (`clientimport`), and never comes peer-to-peer (except with Settings › Testing's switch, off by default:
//! `clienttorrent`). It sends no telemetry: the one count
//! it sends is an anonymous "this mod was installed" (a mod id, a build and a fresh random id per install, never
//! an account or machine id; Settings turns it off: `rankings`). Its network
//! calls: the Workshop catalog and mod zips (`catalog`, `mods`), the Leyline addons listing and zip on the public release (`addons`), the hub's rankings (`rankings`), the relay's realm list, realm lookup and
//! invite knock (`realms`, `host`), the official realm's status file (`status`), the website's forum, read only
//! (`forum`), and GitHub's release list (`update`). Each has an 8 second timeout; the listings fall back to a cached copy.

#[macro_use]
pub mod addresses; // the public names: site, game, relay, new first with the old ones as fallback (docs/DOMAIN-MOVE.md)
pub mod agents; // the Agents page and `Leyline mcp`: the player's own AI assistant, read / ask / never tiers (agents.rs)
pub mod addons; // the Leyline addons: our own in-game addons for a 3.3.5a client (ui/addons.js)
pub mod bugs; // Help › Report a bug: one report to the website's bug list as the signed-in player (bugs.rs)
pub mod branding; // launcher/branding.json: name, look, update sources, release-log key (docs/FORKING.md)
pub mod catalog;
pub mod clientimport; // drop a game folder, a zip or a link to your own backup on the client list (clientimport.rs)
pub mod clienttorrent; // magnets and .torrents for game clients: the owner's testing switch, off by default (clienttorrent.rs)
pub mod clients;
pub mod deeplink; // leyline://install/<id> (hub rankings agent)
pub mod gfx; // Mods & Addons › Graphics: the client's own graphics settings in Config.wtf (gfx.rs)
pub mod forum; // the Forum page: reading the website's forum (stream/forum.py)
pub mod herokey; // Hero Key sign-in to community realms (docs/HERO-KEY-SSO.md; the herokey_sso flag)
pub mod hybrid; // the hybrid client: the desktop game signs in and loads like the browser game (docs/HYBRID-CLIENT.md)
pub mod host;
pub mod launch;
pub mod listed; // listed mods: what players list with "List your mod", installed from the author (stream/listings.py)
pub mod listing; // List your mod from the launcher: check, list, report, edit, remove (stream/listings.py)
pub mod looks; // Looks: graphics packs, UI looks and presets in one click, and back to the original look (looks.rs)
pub mod modes; // the Modes page: game modes one click away (modes agent)
pub mod mods;
pub mod mpq; // a small MPQ writer for the hybrid client's patch archive (our files and Forever UI's glue only)
pub mod news; // the News page: the website's news feed, read only (news.rs)
pub mod net;
pub mod p2p;
pub mod pe_version;
pub mod rankings; // Discover's sort tabs, install counts and ratings (hub rankings agent)
pub mod realms;
pub mod releaselog; // the signed, hash-chained release log ("the chain"; docs/RESILIENCE.md)
pub mod settings;
pub mod social;
pub mod status;
pub mod themes; // theme packs (themes agent)
pub mod update;
pub mod upstream; // upstream addons: curated third-party addons installed from the author's repo (ui/market.js)

#[cfg(feature = "app")]
mod app;
#[cfg(feature = "app")]
pub use app::run;

/// The official realm's realmlist (desktop players): `leylinelauncher.com`. `play.openvanilla.app` reaches the same
/// server forever; `addresses::official_realm()` picks the old one on a computer that cannot resolve the new one yet.
pub const OFFICIAL_REALM: &str = addresses::SITE_HOST;
/// The browser client.
pub const BROWSER_PLAY_URL: &str = crate::site!("/wrath");

/// The only pages the launcher opens in the player's browser, by name. The UI names one; it can never pass a URL.
pub const LINKS: &[(&str, &str)] = &[
    ("play", BROWSER_PLAY_URL),
    // Forge's source (GPL-3.0: the source beside the program; ui/den.js "Get Forge")
    ("forge-source", host::forge::SOURCE_URL),
    ("account", crate::site!("/wrath")),
    // Manage account (ui/account.js): the website's profile page, the Hero Key page, linking another way, linked launchers
    ("profile", crate::site!("/wrath/profile")),
    ("herokey", crate::site!("/account/key")),
    ("linkgoogle", crate::site!("/auth/google/start?link=1")),
    ("linkdiscord", crate::site!("/auth/discord/start?link=1")),
    ("linkedlaunchers", crate::site!("/wrath/link")),
    ("news", crate::site!("/wrath/leyline/news")),
    ("docs", crate::site!("/wrath/docs")),
    ("start", crate::site!("/wrath/start")), // Start here: the four ways in (Help, the tour; stream/wrath_portal.py)
    ("gettingstarted", crate::site!("/wrath/docs/getting-started")), // the player guide (Help, Find your game files)
    ("mods", crate::site!("/wrath/mods")),
    ("discord", "https://discord.gg/hVeUxYwbkr"),
    ("forum", forum::PAGE), // the website's forum: posting happens there (forum.rs)
    ("bugs", crate::site!("/wrath/bugs")), // the website's Report a bug form (signed out: Help › Report a bug)
    ("forumnew", crate::site!("/wrath/leyline/forum/new")),
    ("github", "https://github.com/orionopenwow/Leyline"),
    ("host", host::HOST_PAGE),
    ("hostguide", crate::site!("/wrath/docs/hosting")), // the Host pack and its settings
    ("agentsguide", crate::site!("/wrath/docs/agents")), // Agents and Help: docs/leyline/agents.md
    ("hostdocs", crate::site!("/wrath/docs/hosting")), // the host console's guide (host/console.rs)
    ("alwayson", crate::site!("/wrath/docs/always-on")),
    ("network", crate::site!("/wrath/docs/network")), // the host guide's first item: how the network works
    ("themes", "https://github.com/orionopenwow/WrathUnleashed/blob/main/docs/THEMES.md"), // themes agent
    ("modders", "https://github.com/orionopenwow/WrathUnleashed/blob/main/mods/README-modders.md"),
    ("releases", update::RELEASES_PAGE),
    ("docker", host::docker::DESKTOP_URL),
    ("dockerengine", host::docker::ENGINE_URL),
];

/// The address for a named link, if it is one of `LINKS`.
pub fn link(name: &str) -> Option<&'static str> {
    LINKS.iter().find(|(n, _)| *n == name).map(|(_, u)| *u)
}

#[cfg(test)]
mod link_tests {
    #[test]
    fn only_listed_links_open() {
        assert_eq!(super::link("discord"), Some("https://discord.gg/hVeUxYwbkr"));
        assert!(super::link("https://example.com").is_none());
        assert!(super::LINKS.iter().all(|(_, u)| u.starts_with("https://")));
    }

    #[test]
    fn the_forum_links_to_the_site() {
        assert_eq!(super::link("forum"), Some("https://leylinelauncher.com/wrath/leyline/forum"));
        assert_eq!(super::link("forumnew"), Some("https://leylinelauncher.com/wrath/leyline/forum/new"));
        assert_eq!(super::link("bugs"), Some("https://leylinelauncher.com/wrath/bugs"));
    }

    #[test]
    fn manage_account_links_to_the_site() {
        assert_eq!(super::link("profile"), Some("https://leylinelauncher.com/wrath/profile"));
        assert_eq!(super::link("herokey"), Some("https://leylinelauncher.com/account/key"));
        assert_eq!(super::link("linkgoogle"), Some("https://leylinelauncher.com/auth/google/start?link=1"));
        assert_eq!(super::link("linkdiscord"), Some("https://leylinelauncher.com/auth/discord/start?link=1"));
        assert_eq!(super::link("linkedlaunchers"), Some("https://leylinelauncher.com/wrath/link"));
    }

    #[test]
    fn news_and_the_player_guide_link_to_the_site_and_nothing_asks_for_money() {
        assert_eq!(super::link("news"), Some("https://leylinelauncher.com/wrath/leyline/news"));
        assert_eq!(super::link("gettingstarted"), Some("https://leylinelauncher.com/wrath/docs/getting-started"));
        assert_eq!(super::link("start"), Some("https://leylinelauncher.com/wrath/start"));
        assert!(super::link("support").is_none());
        assert!(super::LINKS.iter().all(|(_, u)| !u.contains("ko-fi") && !u.contains("patreon")));
    }

    #[test]
    fn the_host_guide_links_to_the_site_docs() {
        assert_eq!(super::link("hostdocs"), Some("https://leylinelauncher.com/wrath/docs/hosting"));
        assert_eq!(super::link("hostguide"), Some("https://leylinelauncher.com/wrath/docs/hosting"));
        assert_eq!(super::link("alwayson"), Some("https://leylinelauncher.com/wrath/docs/always-on"));
        assert_eq!(super::link("network"), Some("https://leylinelauncher.com/wrath/docs/network"));
        let mut names: Vec<&str> = super::LINKS.iter().map(|(n, _)| *n).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), super::LINKS.len(), "every name once");
    }
}
