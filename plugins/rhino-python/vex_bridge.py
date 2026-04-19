"""
vex_bridge.py — Rhino-side shell for the vex-bridge daemon.

Drop this file into Rhino's Scripts folder, then bind it to a toolbar
button or menu item. All real work happens in the local daemon at
http://127.0.0.1:7878 — this file is intentionally ~50 LOC.

Tested against Rhino 7+ on macOS and Windows.
"""

from __future__ import print_function

import json
import os
import sys
import urllib2  # Rhino's bundled IronPython 2.7

import Rhino  # noqa
import rhinoscriptsyntax as rs


BRIDGE_URL = "http://127.0.0.1:7878/v1/repo/push"


def _token_path():
    if sys.platform == "win32":
        return os.path.join(os.environ["APPDATA"], "vex-bridge", "access-token")
    return os.path.join(os.path.expanduser("~"), ".config", "vex-bridge", "access-token")


def _read_token():
    try:
        with open(_token_path(), "r") as fh:
            return fh.read().strip()
    except IOError:
        return None


def main():
    token = _read_token()
    if not token:
        rs.MessageBox(
            "vex-bridge is not running, or you have not paired this machine.\n"
            "Open a terminal and run:  vex-bridge pair",
            0,
            "vex-bridge",
        )
        return

    project_id = rs.GetString("Project ID (prj_...)")
    if not project_id:
        return

    body = json.dumps({"project_id": project_id, "branch": "main"})
    req = urllib2.Request(
        BRIDGE_URL,
        data=body,
        headers={
            "Content-Type": "application/json",
            "X-Vex-Bridge-Token": token,
        },
    )
    try:
        resp = urllib2.urlopen(req, timeout=120)
        rs.MessageBox("Pushed: " + resp.read(), 0, "vex-bridge")
    except urllib2.HTTPError as e:
        rs.MessageBox("Push failed: HTTP %d\n%s" % (e.code, e.read()), 0, "vex-bridge")
    except urllib2.URLError as e:
        rs.MessageBox("Cannot reach vex-bridge daemon: %s" % e.reason, 0, "vex-bridge")


if __name__ == "__main__":
    main()
