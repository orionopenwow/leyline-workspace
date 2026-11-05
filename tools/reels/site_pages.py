#!/usr/bin/env python3
"""The website's own pages, rendered here from stream/wrath_portal.py (no server, nothing fetched), for the reel kit's
site and phone takes (takes.cjs serves them as https://leylinelauncher.com/... with the art from stream/www-files):
the Leyline page, the realms page, Start here (its four cards), and the browser game's line as a player in it sees it (your place
and the wait; never the seat numbers, owner 4 Nov).

    python3 -I tools/reels/site_pages.py <out dir>      writes <path>.html per page and pages.json (path -> file)
"""
import importlib.util
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
PORTAL = os.path.join(ROOT, "stream", "wrath_portal.py")

# the site path a take opens -> how it is made (the queue page has no fixed address: it shows only in a line)
QUEUE_PREVIEW = "/wrath/queue-preview"


def portal():
    spec = importlib.util.spec_from_file_location("wrath_portal_reels", PORTAL)
    wp = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(wp)
    return wp


def pages(wp):
    return {
        "/wrath/leyline": wp.leyline_page,
        "/wrath/leyline/realms": lambda: wp.realms_page({}),
        "/wrath/start": wp.start_page,
        # a player third in line, about 4 minutes: what the page says to them (no token: no script polling a server)
        QUEUE_PREVIEW: lambda: wp.queue_page({"state": "line", "position": 3, "wait_min": 4}, "reel", "/wrath/play"),
    }


def main(argv):
    out = os.path.abspath(argv[0] if argv else os.path.join(HERE, "takes", "_site"))
    os.makedirs(out, exist_ok=True)
    wp = portal()
    index = {}
    for path, make in pages(wp).items():
        body = make()
        if not body:
            continue
        name = path.strip("/").replace("/", "_") + ".html"
        with open(os.path.join(out, name), "wb") as f:
            f.write(body)
        index[path] = name
    with open(os.path.join(out, "pages.json"), "w", encoding="utf-8") as f:
        json.dump(index, f, indent=1)
    print("site pages: %d in %s" % (len(index), out))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
