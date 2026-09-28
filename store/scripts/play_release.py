#!/usr/bin/env python3
"""Google Play release helper (no browser).

  upload <aab> <ver>          create an edit, upload the bundle, stage it on the production
                              track with release notes from store/release-notes/<ver>.md
                              (the edit is NOT committed — nothing is published)
  commit <edit id>            commit a staged edit (this publishes to production)
  discard <edit id>           throw a staged edit away

Service account: ~/.playconsole/play-publisher.json (never printed or committed).
"""
import json, sys, time, urllib.request, urllib.error, pathlib
import jwt

PKG = "com.parkjeongseop.wipi"
BASE = f"https://androidpublisher.googleapis.com/androidpublisher/v3/applications/{PKG}"
NOTES = pathlib.Path(__file__).resolve().parents[1] / "release-notes"


def token():
    sa = json.loads((pathlib.Path.home() / ".playconsole/play-publisher.json").read_text())
    now = int(time.time())
    assertion = jwt.encode({"iss": sa["client_email"], "scope": "https://www.googleapis.com/auth/androidpublisher",
                            "aud": "https://oauth2.googleapis.com/token", "iat": now, "exp": now + 3600}, sa["private_key"], algorithm="RS256")
    data = f"grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer&assertion={assertion}".encode()
    with urllib.request.urlopen(urllib.request.Request("https://oauth2.googleapis.com/token", data=data)) as r:
        return json.loads(r.read())["access_token"]


def api(method, url, body=None, raw=None, ctype="application/json"):
    req = urllib.request.Request(url, method=method, headers={"Authorization": f"Bearer {token()}", "Content-Type": ctype},
                                 data=raw if raw is not None else (json.dumps(body).encode() if body is not None else None))
    try:
        with urllib.request.urlopen(req) as r:
            return json.loads(r.read() or b"{}")
    except urllib.error.HTTPError as e:
        raise SystemExit(f"{method} {url} -> {e.code}: {e.read().decode()[:800]}")


def notes(ver):
    text = (NOTES / f"{ver}.md").read_text()
    ko = text.split("## 한국어", 1)[1].split("\n## ", 1)[0].strip().split("\n", 1)[1].strip()
    en = text.split("## English", 1)[1].strip()
    return [{"language": "ko-KR", "text": ko[:500]}, {"language": "en-US", "text": en[:500]}]


def upload(aab, ver):
    edit = api("POST", f"{BASE}/edits", {})["id"]
    print("edit", edit)
    bundle = api("POST", f"https://androidpublisher.googleapis.com/upload/androidpublisher/v3/applications/{PKG}/edits/{edit}/bundles?uploadType=media",
                 raw=pathlib.Path(aab).read_bytes(), ctype="application/octet-stream")
    print("uploaded versionCode", bundle["versionCode"])
    api("PUT", f"{BASE}/edits/{edit}/tracks/production",
        {"track": "production", "releases": [{"name": ver, "versionCodes": [str(bundle["versionCode"])], "status": "completed", "releaseNotes": notes(ver)}]})
    print("staged on production (not committed). commit with:", f"play_release.py commit {edit}")


def commit(edit):
    print(api("POST", f"{BASE}/edits/{edit}" + ":commit", {}))


def discard(edit):
    api("DELETE", f"{BASE}/edits/{edit}"); print("discarded", edit)


if __name__ == "__main__":
    cmd = sys.argv[1:]
    {"upload": lambda: upload(cmd[1], cmd[2]), "commit": lambda: commit(cmd[1]), "discard": lambda: discard(cmd[1])}[cmd[0]]()
