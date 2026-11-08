"""Checks a built mdBook for broken relative links.

Usage: check-links.py <the book's output directory>

Every relative `href`/`src` in the pages must name a file of the output,
and every `#fragment` an `id` of the page it points at. External links
(any scheme, `//host`) are left alone. Exits 1 and lists the broken links
when there are any.
"""

import sys
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit

# NOTE: mdBook's own pages: the print page rewrites every link to its anchors
SKIP = {"print.html", "toc.html", "404.html"}


class Page(HTMLParser):
    """The ids a page declares and the links it makes."""

    def __init__(self):
        super().__init__()
        self.ids = set()
        self.links = []

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        for key in ("id", "name"):
            if attrs.get(key):
                self.ids.add(attrs[key])
        for key in ("href", "src"):
            if attrs.get(key):
                self.links.append(attrs[key])


def parse(path):
    page = Page()
    page.feed(path.read_text(encoding="utf-8"))
    return page


def main(out):
    pages = {p: parse(p) for p in out.rglob("*.html")}
    broken = []
    for path, page in pages.items():
        if path.name in SKIP:
            continue
        for link in page.links:
            parts = urlsplit(link)
            if parts.scheme or link.startswith("//"):
                continue
            target = (path.parent / unquote(parts.path)).resolve() if parts.path else path
            if not target.exists():
                broken.append(f"{path.relative_to(out)}: {link} (no such file)")
                continue
            if parts.fragment and target.suffix == ".html":
                ids = pages[target].ids if target in pages else parse(target).ids
                if unquote(parts.fragment) not in ids:
                    broken.append(f"{path.relative_to(out)}: {link} (no such heading)")
    for line in sorted(set(broken)):
        print(line)
    return 1 if broken else 0


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1]).resolve()))
