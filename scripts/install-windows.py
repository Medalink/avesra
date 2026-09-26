"""Explicit portable package creation and current-user transport registration.

Requires Python 3.12+. Never launches the companion, browser or native host.
"""
import argparse
import ctypes
from ctypes import wintypes
import json
import os
from pathlib import Path
import re
import shutil
import sqlite3
import subprocess
import sys

sys.dont_write_bytecode = True
from package_common import atomic_json, digest, exclusive_json, files, plain, read, sha, verify

HOST = "com.avesra.companion"
KEYS = {
    "chrome": "Software\\Google\\Chrome\\NativeMessagingHosts\\" + HOST,
    "brave": "Software\\BraveSoftware\\Brave-Browser\\NativeMessagingHosts\\" + HOST,
}
EXTRAS = ("com.avesra.companion.json", "avesra-extension-origin.txt", "installation.json")


def package(args):
    sha(args.source)
    source_root = Path(__file__).parent.parent
    store = read(source_root / "crates/avesra-core/src/store.rs", 262144).decode()
    if re.findall(r"INSERT INTO schema_version VALUES\((\d+)\)", store) != [str(args.store_schema)]:
        raise ValueError("Package schema must match the frozen source used for the built executables")
    output = plain(args.output)
    if output.exists():
        raise ValueError("Package destination must not exist")
    release, extension = plain(args.release), plain(args.extension)
    payload = [(release / name, name) for name in ("avesra-desktop.exe", "avesra-native-host.exe")]
    payload += [(extension / name, "extension/" + name) for name in files(extension)]
    payload.append((Path(__file__).parent.parent / "docs/deployment-lifecycle.md", "runbook.md"))
    payload += [(Path(__file__).parent / name, name) for name in ("install-windows.py", "package_common.py")]
    if len(payload) > 512:
        raise ValueError("Package file limit")
    # Validate all source paths before creating a destination.
    for path, _ in payload:
        digest(path)
    output.mkdir(parents=False)
    members = []
    for path, name in payload:
        target = output / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(plain(path), target)
        members.append({"path": name, "bytes": target.stat().st_size, "sha256": digest(target)})
    exclusive_json(output / "package.json", {"version": 1, "source": args.source, "store_schema": args.store_schema, "files": members})
    expected = digest(output / "package.json")
    verify(output, expected)
    print("Package manifest SHA256:", expected)
    print("Keep this hash with the trusted release record. Package was not activated.")


def registry(browser):
    import winreg
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, KEYS[browser]) as key:
            value, kind = winreg.QueryValueEx(key, "")
            if kind != winreg.REG_SZ or not isinstance(value, str):
                raise ValueError("Unsupported native-host registration")
            if winreg.QueryInfoKey(key)[:2] != (0, 1):
                raise ValueError("Native-host registration has unrelated content")
            return value
    except FileNotFoundError:
        return None


def write_registry(browser, value):
    import winreg
    if value is None:
        if registry(browser) is not None:
            winreg.DeleteKey(winreg.HKEY_CURRENT_USER, KEYS[browser])
    else:
        with winreg.CreateKeyEx(winreg.HKEY_CURRENT_USER, KEYS[browser], 0, winreg.KEY_SET_VALUE) as key:
            winreg.SetValueEx(key, "", 0, winreg.REG_SZ, value)


def closed():
    # Metadata-only process inventory; no stop, wait, app launch or focus.
    class Entry(ctypes.Structure):
        _fields_ = [("size", wintypes.DWORD), ("usage", wintypes.DWORD), ("pid", wintypes.DWORD),
                    ("heap", ctypes.c_size_t), ("module", wintypes.DWORD), ("threads", wintypes.DWORD),
                    ("parent", wintypes.DWORD), ("priority", wintypes.LONG), ("flags", wintypes.DWORD),
                    ("exe", wintypes.WCHAR * 260)]
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
    kernel.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
    for name in ("Process32FirstW", "Process32NextW"):
        function = getattr(kernel, name)
        function.argtypes = [wintypes.HANDLE, ctypes.POINTER(Entry)]
        function.restype = wintypes.BOOL
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    handle = kernel.CreateToolhelp32Snapshot(2, 0)
    if handle == ctypes.c_void_p(-1).value:
        raise ValueError("Cannot inspect running companion processes")
    try:
        entry = Entry()
        entry.size = ctypes.sizeof(entry)
        present = kernel.Process32FirstW(handle, ctypes.byref(entry))
        while present:
            if entry.exe.lower() in {"avesra-desktop.exe", "avesra-native-host.exe"}:
                raise ValueError("Close the companion/native host normally before activation")
            present = kernel.Process32NextW(handle, ctypes.byref(entry))
        if ctypes.get_last_error() != 18:  # ERROR_NO_MORE_FILES
            raise ValueError("Incomplete process inventory")
    finally:
        kernel.CloseHandle(handle)


def installed(root, name):
    if not re.fullmatch(r"[0-9a-f]{64}-[a-p]{32}", name):
        raise ValueError("Invalid retained version")
    location = plain(root / "versions" / name)
    record = json.loads(read(location / "installation.json", 4096))
    if set(record) != {"version", "manifest", "extension"} or record["version"] != 1 or name != record["manifest"] + "-" + record["extension"]:
        raise ValueError("Invalid installed package record")
    manifest = verify(location, record["manifest"], EXTRAS)
    expected = {"name": HOST, "description": "Avesra companion", "path": str(location / "avesra-native-host.exe"),
                "type": "stdio", "allowed_origins": ["chrome-extension://" + record["extension"] + "/"]}
    if json.loads(read(location / EXTRAS[0], 4096)) != expected or read(location / EXTRAS[1], 256) != (expected["allowed_origins"][0] + "\n").encode():
        raise ValueError("Native transport installation changed")
    return location, manifest


def stage(root, args):
    source = plain(args.package)
    verify(source, args.sha256)
    if not re.fullmatch(r"[a-p]{32}", args.extension_id):
        raise ValueError("Use the actual installed 32-letter extension ID")
    name = args.sha256 + "-" + args.extension_id
    destination = root / "versions" / name
    if destination.exists():
        installed(root, name)
        print("Already staged:", name)
        return
    temporary = root / "versions" / (name + ".staging")
    if temporary.exists():
        raise ValueError("Incomplete stage retained; inspect it before retrying")
    shutil.copytree(source, temporary)
    verify(temporary, args.sha256)
    origin = "chrome-extension://" + args.extension_id + "/"
    exclusive_json(temporary / EXTRAS[0], {"name": HOST, "description": "Avesra companion",
        "path": str(destination / "avesra-native-host.exe"), "type": "stdio", "allowed_origins": [origin]})
    with (temporary / EXTRAS[1]).open("xb") as output:
        output.write((origin + "\n").encode())
    exclusive_json(temporary / EXTRAS[2], {"version": 1, "manifest": args.sha256, "extension": args.extension_id})
    os.rename(temporary, destination)
    installed(root, name)
    print("Staged immutable version:", name)
    print("No process, registration, pairing or store was changed.")


def extension_files(manifest):
    return [member for member in manifest["files"] if member["path"].startswith("extension/")]


def check_extension(root, members):
    directory = plain(root / "browser-extension")
    if set(files(directory)) != {v["path"][10:] for v in members}:
        raise ValueError("Stable extension files differ from selected package")
    for item in members:
        target = directory / item["path"][10:]
        if target.stat().st_size != item["bytes"] or digest(target) != item["sha256"]:
            raise ValueError("Stable extension hash differs from selected package")


def prepare_extension(root, args):
    closed()
    owner = state(root)
    if owner["registrations"] or any(registry(browser) is not None for browser in KEYS):
        raise ValueError("Unregister the owned integration before changing extension files")
    source = plain(args.package)
    manifest = verify(source, args.sha256)
    members = extension_files(manifest)
    directory = root / "browser-extension"
    marker = root / "extension.json"
    saved = None
    if directory.exists():
        saved = json.loads(read(marker, 262144))
        if set(saved) != {"version", "manifest", "files"} or saved["version"] != 1:
            raise ValueError("Stable extension ownership is unknown")
        check_extension(root, saved["files"])
        if saved["manifest"] == args.sha256:
            check_extension(root, members)
            print("Stable extension already matches the selected package:", directory)
            return
    elif marker.exists():
        raise ValueError("Stable extension is incomplete")
    pending = root / "browser-extension.pending"
    pending.mkdir()
    for item in members:
        target = pending / item["path"][10:]
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source / item["path"], target)
        if digest(target) != item["sha256"]:
            raise ValueError("Extension changed during copy")
    replacement = {"version": 1, "manifest": args.sha256, "files": members}
    exclusive_json(root / "extension-activation.json", {"version": 1, "before": saved, "after": replacement})
    # Keep the old extension intact under a unique hash path. Never remove it.
    if directory.exists():
        retained = root / ("browser-extension.previous-" + sha(saved["manifest"]))
        if retained.exists():
            raise ValueError("Retained extension directory already exists; inspect before retrying")
        os.rename(directory, retained)
    os.rename(pending, directory)
    atomic_json(marker, replacement)
    check_extension(root, members)
    (root / "extension-activation.json").unlink()
    print("Load/reload the unpacked extension ONLY from this stable directory:", directory)
    print("Read its actual browser ID, then stage/activate the package. Browser pairing remains explicit.")


def recover_extension(root):
    closed()
    if state(root)["registrations"] or any(registry(browser) is not None for browser in KEYS):
        raise ValueError("Extension recovery requires unregistered integration")
    journal_path = root / "extension-activation.json"
    journal = json.loads(read(journal_path, 524288))
    if set(journal) != {"version", "before", "after"} or journal["version"] != 1:
        raise ValueError("Invalid extension recovery journal")
    before, after = journal["before"], journal["after"]
    directory = root / "browser-extension"
    if directory.exists():
        if before is not None:
            try:
                check_extension(root, before["files"])
                atomic_json(root / "extension.json", before)
                journal_path.unlink()
                print("Previous stable extension remains intact; pending copy retained.")
                return
            except ValueError:
                pass
        check_extension(root, after["files"])
        retained = root / ("browser-extension.interrupted-" + sha(after["manifest"]))
        if retained.exists():
            raise ValueError("Interrupted extension already retained; inspect it explicitly")
        os.rename(directory, retained)
    if before is not None:
        previous = root / ("browser-extension.previous-" + sha(before["manifest"]))
        if not previous.exists():
            raise ValueError("Previous extension missing; recovery remains pending")
        os.rename(previous, directory)
        check_extension(root, before["files"])
        atomic_json(root / "extension.json", before)
    elif (root / "extension.json").exists():
        (root / "extension.json").unlink()
    journal_path.unlink()
    print("Prior stable extension restored; all interrupted files were retained.")


def shortcut_path():
    return plain(Path(os.environ["APPDATA"]) / "Microsoft/Windows/Start Menu/Programs/Avesra.lnk")


def shortcut(target=None, *, inspect=False):
    path = shortcut_path()
    if inspect and not path.exists():
        return None
    if target is None and not inspect:
        if path.exists():
            path.unlink()
        return None
    temporary = path.with_name("Avesra.installing.lnk")
    if not inspect and temporary.exists():
        raise ValueError("Incomplete owned shortcut retained")
    path.parent.mkdir(parents=True, exist_ok=True)
    script = ('$ErrorActionPreference="Stop"; [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); '
              '$d=[Console]::In.ReadToEnd()|ConvertFrom-Json; $s=New-Object -ComObject WScript.Shell; '
              '$l=$s.CreateShortcut($d.path); if($d.inspect){[Console]::Write($l.TargetPath)}'
              'else{$l.TargetPath=$d.target; $l.WorkingDirectory=$d.directory; $l.Save()}')
    data = {"path": str(path if inspect else temporary), "inspect": inspect,
            "target": target, "directory": str(Path(target).parent) if target else None}
    result = subprocess.run([str(Path(os.environ["SYSTEMROOT"]) / "System32/WindowsPowerShell/v1.0/powershell.exe"),
                             "-NoProfile", "-NonInteractive", "-Command", script],
                            input=json.dumps(data).encode(), stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                            timeout=10, creationflags=0x08000000)
    if result.returncode or len(result.stdout) > 4096:
        raise ValueError("Owned shortcut operation failed")
    if inspect:
        return result.stdout.decode().strip()
    os.replace(temporary, path)


def desktop(root, record):
    return str(root / "versions" / record["current"] / "avesra-desktop.exe") if record["current"] else None


def schemas(data, maximum):
    data = plain(data)
    expected = plain(Path(os.environ["APPDATA"]) / "com.avesra.desktop")
    if data != expected:
        raise ValueError("Use the actual Avesra app-data directory, not an empty substitute")
    for name in ("avesra.db", "native-actions.db"):
        path = plain(data / name)
        if not path.exists():
            continue
        if not path.is_file():
            raise ValueError("Invalid existing store")
        connection = sqlite3.connect(path.as_uri() + "?mode=ro", uri=True, timeout=2)
        try:
            connection.execute("PRAGMA query_only=ON")
            values = connection.execute("SELECT version FROM schema_version LIMIT 2").fetchall()
            if len(values) != 1 or type(values[0][0]) is not int or not 1 <= values[0][0] <= maximum:
                raise ValueError("Retained package is incompatible with existing store schema")
        finally:
            connection.close()


def state(root):
    path = root / "active.json"
    if not path.exists():
        return {"version": 1, "current": None, "previous": None, "registrations": {}}
    value = json.loads(read(path, 16384))
    if set(value) != {"version", "current", "previous", "registrations"} or value["version"] != 1:
        raise ValueError("Invalid installation ownership record")
    if not isinstance(value["registrations"], dict) or not set(value["registrations"]) <= set(KEYS):
        raise ValueError("Invalid registration ownership")
    if value["current"] is not None:
        location, _ = installed(root, value["current"])
        if any(path != str(location / EXTRAS[0]) for path in value["registrations"].values()):
            raise ValueError("Owned registration does not match current package")
    elif value["registrations"]:
        raise ValueError("Registration has no package owner")
    return value


def recover(root):
    closed()
    path = root / "activation.json"
    transaction = json.loads(read(path, 32768))
    if set(transaction) != {"version", "before", "after", "old_state"} or transaction["version"] != 1:
        raise ValueError("Invalid recovery journal")
    # Only values in this exact interrupted transaction may be reverted.
    for browser in KEYS:
        if registry(browser) not in (transaction["before"].get(browser), transaction["after"].get(browser)):
            raise ValueError("Registration changed externally; recovery refused")
    old = transaction["old_state"]
    if old["current"] is not None:
        installed(root, old["current"])
    for browser in KEYS:
        if transaction["before"].get(browser) is not None and (old["current"] is None or transaction["before"][browser] != str(root / "versions" / old["current"] / EXTRAS[0])):
            raise ValueError("Recovery registration lacks its prior package owner")
    current_link = shortcut(inspect=True)
    permitted_links = {desktop(root, old)}
    for target in transaction["after"].values():
        if target:
            permitted_links.add(str(Path(target).parent / "avesra-desktop.exe"))
    permitted_links.add(None)
    if current_link not in permitted_links:
        raise ValueError("Shortcut changed externally; recovery refused")
    for browser in KEYS:
        write_registry(browser, transaction["before"].get(browser))
    shortcut(desktop(root, old))
    atomic_json(root / "active.json", old)
    path.unlink()
    print("Prior exact owned registrations restored. Stores were not touched.")


def activate(root, args):
    closed()
    old = state(root)
    before = {browser: registry(browser) for browser in KEYS}
    if any(before[browser] != old["registrations"].get(browser) for browser in KEYS):
        raise ValueError("Unrelated or changed native-host registration; refusing overwrite")
    if shortcut(inspect=True) != desktop(root, old):
        raise ValueError("Unrelated or changed Avesra shortcut; refusing overwrite")
    if args.command == "unregister":
        new = {"version": 1, "current": None, "previous": old["current"] or old["previous"], "registrations": {}}
    else:
        name = args.version
        if name is None:
            raise ValueError("No retained prior package")
        location, manifest = installed(root, name)
        check_extension(root, extension_files(manifest))
        schemas(args.data, manifest["store_schema"])
        if old["registrations"] and old["current"].split("-")[1] != name.split("-")[1]:
            raise ValueError("Registered browser has a different extension ID; unregister before switching")
        browsers = set(old["registrations"])
        browsers.add(args.browser)
        if not browsers:
            raise ValueError("Select a browser through activate")
        new = {"version": 1, "current": name, "previous": old["current"] or old["previous"],
               "registrations": {browser: str(location / EXTRAS[0]) for browser in browsers}}
    after = {browser: new["registrations"].get(browser) for browser in KEYS}
    exclusive_json(root / "activation.json", {"version": 1, "before": before, "after": after, "old_state": old})
    for browser in KEYS:
        write_registry(browser, after[browser])
    shortcut(desktop(root, new))
    atomic_json(root / "active.json", new)
    (root / "activation.json").unlink()
    print("Registration updated. No application was launched; browser pairing remains explicit.")
    if new["current"]:
        print("Use the Avesra Start menu shortcut for the matched desktop/native-host package.")


def main():
    if sys.version_info < (3, 12):
        raise ValueError("Python 3.12 or newer is required")
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    create = commands.add_parser("package")
    for name in ("source", "output", "release", "extension"):
        create.add_argument("--" + name, required=True)
    create.add_argument("--store-schema", type=int, choices=(13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27), required=True)
    staged = commands.add_parser("stage")
    for name in ("package", "sha256", "extension-id"):
        staged.add_argument("--" + name, required=True)
    extension = commands.add_parser("prepare-extension")
    extension.add_argument("--package", required=True)
    extension.add_argument("--sha256", required=True)
    active = commands.add_parser("activate")
    active.add_argument("--version", required=True)
    active.add_argument("--browser", choices=KEYS, required=True)
    active.add_argument("--data", required=True, help="Actual companion app-data directory; inspected read-only")
    rollback = commands.add_parser("rollback")
    rollback.add_argument("--version", required=True, help="Explicit retained rollback target, recorded before unregister")
    rollback.add_argument("--data", required=True)
    rollback.add_argument("--browser", choices=KEYS, required=True)
    commands.add_parser("unregister")
    commands.add_parser("recover")
    commands.add_parser("recover-extension")
    args = parser.parse_args()
    if args.command == "package":
        return package(args)
    if sys.platform != "win32":
        raise ValueError("Current-user installation requires Windows")
    root = plain(Path(os.environ["LOCALAPPDATA"]) / "Avesra" / "packages")
    root.mkdir(parents=True, exist_ok=True)
    (root / "versions").mkdir(exist_ok=True)
    # The OS releases this byte-range lock after a crash; the journal survives.
    import msvcrt
    lock = root / "installer.lock"
    owner = lock.open("a+b")
    if owner.tell() == 0:
        owner.write(b"0")
        owner.flush()
    owner.seek(0)
    msvcrt.locking(owner.fileno(), msvcrt.LK_NBLCK, 1)
    try:
        if (root / "activation.json").exists() and args.command != "recover":
            raise ValueError("Interrupted activation: run recover before another mutation")
        if (root / "extension-activation.json").exists() and args.command != "recover-extension":
            raise ValueError("Interrupted extension replacement: run recover-extension first")
        if args.command == "stage":
            stage(root, args)
        elif args.command == "prepare-extension":
            prepare_extension(root, args)
        elif args.command == "recover":
            recover(root)
        elif args.command == "recover-extension":
            recover_extension(root)
        else:
            activate(root, args)
    finally:
        owner.seek(0)
        msvcrt.locking(owner.fileno(), msvcrt.LK_UNLCK, 1)
        owner.close()


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print("Installation refused:", error, file=sys.stderr)
        raise SystemExit(1)
