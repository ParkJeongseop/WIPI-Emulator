#!/usr/bin/env python3
"""App Store Connect release helper (no browser).

Steps (each idempotent, run in order):
  status                 show versions/builds for the app
  prepare  <ver> <build> wait for the uploaded build, mark usesNonExemptEncryption=false,
                         create/select App Store version <ver>, attach the build, set What's New
  submit   <ver>         create a review submission for the version and submit it

Upload the IPA first with:
  xcrun altool --upload-app -f <ipa> -t ios --apiKey HC4VSSZNW8 --apiIssuer <issuer>
Key: ~/.appstoreconnect/private_keys/AuthKey_HC4VSSZNW8.p8 (never printed or committed).
"""
import json, sys, time, urllib.request, urllib.error, pathlib
import jwt

KEY_ID = "HC4VSSZNW8"
ISSUER = "58293299-0864-47ef-be08-aa732a45c1e7"
APP_ID = "6793132618"
BASE = "https://api.appstoreconnect.apple.com/v1"
NOTES = pathlib.Path(__file__).resolve().parents[1] / "release-notes"


def token():
    key = (pathlib.Path.home() / ".appstoreconnect/private_keys" / f"AuthKey_{KEY_ID}.p8").read_text()
    now = int(time.time())
    return jwt.encode({"iss": ISSUER, "iat": now, "exp": now + 1200, "aud": "appstoreconnect-v1"}, key, algorithm="ES256", headers={"kid": KEY_ID})


def api(method, path, body=None):
    req = urllib.request.Request(BASE + path if path.startswith("/") else path, method=method,
                                 headers={"Authorization": f"Bearer {token()}", "Content-Type": "application/json"},
                                 data=json.dumps(body).encode() if body is not None else None)
    try:
        with urllib.request.urlopen(req) as r:
            return json.loads(r.read() or b"{}")
    except urllib.error.HTTPError as e:
        raise SystemExit(f"{method} {path} -> {e.code}: {e.read().decode()[:800]}")


def whats_new(ver):
    text = (NOTES / f"{ver}.md").read_text()
    def section(head):
        body = text.split(head, 1)[1].split("\n## ", 1)[0]
        return "\n".join(l for l in body.strip().splitlines()[1:] if l.strip()).strip() if body.strip().startswith("(") is False else body
    ko = text.split("## 한국어", 1)[1].split("\n## ", 1)[0].strip().split("\n", 1)[1].strip()
    en = text.split("## English", 1)[1].strip()
    return {"ko": ko, "en-US": en}


def status():
    for v in api("GET", f"/apps/{APP_ID}/appStoreVersions?limit=5")["data"]:
        a = v["attributes"]; print("version", a["versionString"], a["appStoreState"], v["id"])
    for b in api("GET", f"/builds?filter[app]={APP_ID}&sort=-uploadedDate&limit=5")["data"]:
        a = b["attributes"]; print("build", a["version"], a["processingState"], a.get("usesNonExemptEncryption"), b["id"])


def prepare(ver, build_no):
    for _ in range(60):
        builds = [b for b in api("GET", f"/builds?filter[app]={APP_ID}&filter[version]={build_no}&sort=-uploadedDate&limit=5")["data"]]
        if builds and builds[0]["attributes"]["processingState"] == "VALID":
            build = builds[0]; break
        print("build", build_no, "not ready:", builds[0]["attributes"]["processingState"] if builds else "not uploaded yet"); time.sleep(60)
    else:
        raise SystemExit("build never became VALID")
    if build["attributes"].get("usesNonExemptEncryption") is not False:
        api("PATCH", f"/builds/{build['id']}", {"data": {"type": "builds", "id": build["id"], "attributes": {"usesNonExemptEncryption": False}}})
        print("usesNonExemptEncryption=false")
    versions = [v for v in api("GET", f"/apps/{APP_ID}/appStoreVersions?filter[versionString]={ver}")["data"]]
    if versions:
        version = versions[0]
    else:
        version = api("POST", "/appStoreVersions", {"data": {"type": "appStoreVersions", "attributes": {"platform": "IOS", "versionString": ver},
                                                             "relationships": {"app": {"data": {"type": "apps", "id": APP_ID}}}}})["data"]
        print("created version", ver)
    print("version", ver, version["attributes"]["appStoreState"], version["id"])
    api("PATCH", f"/appStoreVersions/{version['id']}/relationships/build", {"data": {"type": "builds", "id": build["id"]}})
    print("attached build", build_no)
    notes = whats_new(ver)
    locs = api("GET", f"/appStoreVersions/{version['id']}/appStoreVersionLocalizations")["data"]
    for loc in locs:
        locale = loc["attributes"]["locale"]
        if locale in notes:
            api("PATCH", f"/appStoreVersionLocalizations/{loc['id']}", {"data": {"type": "appStoreVersionLocalizations", "id": loc["id"], "attributes": {"whatsNew": notes[locale]}}})
            print("whatsNew set for", locale)
    detail = api("GET", f"/appStoreVersions/{version['id']}/appStoreReviewDetail").get("data")
    print("review notes present:", bool(detail and detail["attributes"].get("notes")), "(kept as-is; contains the GAME/SOFTWARE INDEX section)")


def submit(ver):
    version = api("GET", f"/apps/{APP_ID}/appStoreVersions?filter[versionString]={ver}")["data"][0]
    sub = api("POST", "/reviewSubmissions", {"data": {"type": "reviewSubmissions", "attributes": {"platform": "IOS"},
                                                      "relationships": {"app": {"data": {"type": "apps", "id": APP_ID}}}}})["data"]
    api("POST", "/reviewSubmissionItems", {"data": {"type": "reviewSubmissionItems",
                                                    "relationships": {"reviewSubmission": {"data": {"type": "reviewSubmissions", "id": sub["id"]}},
                                                                      "appStoreVersion": {"data": {"type": "appStoreVersions", "id": version["id"]}}}}})
    api("PATCH", f"/reviewSubmissions/{sub['id']}", {"data": {"type": "reviewSubmissions", "id": sub["id"], "attributes": {"submitted": True}}})
    print("submitted", ver, sub["id"])


if __name__ == "__main__":
    cmd = sys.argv[1:] or ["status"]
    {"status": lambda: status(), "prepare": lambda: prepare(cmd[1], cmd[2]), "submit": lambda: submit(cmd[1])}[cmd[0]]()
