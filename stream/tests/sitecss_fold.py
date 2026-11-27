"""Tests read a page's styles in the page: since the hub frame's shared styles are a linked stylesheet
(/wrath/art/site.css?v=..., audit 12 Nov), fold it back in where the <link> stands, as the browser applies it.
The link itself is checked on its own (test_site.py, "site.css")."""
import re

LINK_RE = re.compile(rb'<link rel="stylesheet" href="(/wrath/art/site\.css\?v=[0-9a-f]+)">')


def fold(body: bytes, fetch) -> bytes:
    """fetch(path) -> bytes of the stylesheet. Unchanged when the page has no link."""
    m = LINK_RE.search(body or b"")
    if not m:
        return body
    css = fetch(m.group(1).decode())
    return body[:m.start()] + b"<style>" + css + b"</style>" + body[m.end():]
