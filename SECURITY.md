# Security policy

## Reporting a problem

**Please report security problems privately, not in a public issue, pull request or chat.**

Use GitHub's private vulnerability reporting: open the repository's **Security** tab, choose
**Report a vulnerability**, and describe:

* what is affected (the launcher, a relay, the host kit, the website, a mod package, the protocol);
* how to reproduce it, and what an attacker gains;
* any fix you suggest.

Only the maintainers see the report. We confirm we have it, work on a fix in a private advisory, and credit you when
it is published, unless you ask us not to.

`TODO(owner)`: how fast we promise a first answer (for example "within 3 days"), and a second private contact for
when GitHub is unavailable.

## What counts

* Anything that lets someone forge or tamper with signed records (realm, relay, endpoint, block, info), Hero Keys,
  friends messages or the release log; see [docs/COMMONHOLD-V1.md](docs/COMMONHOLD-V1.md).
* A way to make a relay or launcher carry game data, run code it shouldn't, or leak a player's address or secrets.
* A mod package or mode that gets past `wu-mod.py check` while doing something its permissions don't allow.
* Weaknesses in the website or sign-in.

Things already documented as known gaps (for example, that a relay sees desktop game traffic in the clear until the
sealed path exists: docs/LEYLINE-MESH.md §9) are still welcome as reports if you find a new way to exploit them.

## Please don't

* test against other players' realms or accounts, or our servers beyond what you need to show the problem;
* run denial-of-service tests against the public relays;
* include game files in a report.

## Supported versions

Only the newest release of the launcher, the relay and the host kit gets security fixes. The launcher updates
itself; hosts update with the Host pack.
