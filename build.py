#!/usr/bin/env python3
"""Builds the POC404 launcher as a Windows .exe, with your icon, from a black window.

    python build.py                      the window
    python build.py --build              the same build with no window (for scripts)
    python build.py --apply-update NEW   takes a newer project (a .zip or a folder) in: its
                                         backend replaces ours, the interface and the icon stay

What it does:
  1. Turns the icon picture (packaging/icons/app-icon-source.png, or one you browse for) into
     packaging/icons/horizon.ico, every size Windows asks for. The original horizon.ico is kept
     once as horizon.ico.orig.
  2. Runs `cargo build --release` in launcher/ (launcher/build.rs puts that .ico in the .exe).
  3. Copies the result to dist/POC404.exe.

Branding and updates: every build first puts the POC404 name, the icon and the "the launcher never
replaces itself" rule (BRANDING below) back, so a newer project dropped over this one can't bring
the old name, icon or interface back. `--apply-update` does the same on purpose: it copies
everything from the newer project except the files in PROTECTED (the interface and the icon).

Needs: Python 3.9+, Pillow (`pip install pillow`) and Rust (https://rustup.rs), on Windows.
"""
from __future__ import annotations

import os
import queue
import re
import shutil
import subprocess
import sys
import threading
from pathlib import Path

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")  # Windows CI defaults to cp1252
    except (AttributeError, ValueError):
        pass

ROOT = Path(__file__).resolve().parent
LAUNCHER = ROOT / "launcher"
ICONS = ROOT / "packaging" / "icons"
DEFAULT_SOURCE = ICONS / "app-icon-source.png"
ICO = ICONS / "horizon.ico"
SIZES = [16, 24, 32, 48, 64, 128, 256]
CRATE = "horizon-launcher"
NAME = "POC404"
IS_WINDOWS = os.name == "nt"
EXE = f"{CRATE}.exe" if IS_WINDOWS else CRATE


def read_text(path: Path) -> str:
    with open(path, encoding="utf-8", newline="") as f:
        return f.read()


def write_text(path: Path, text: str) -> None:
    with open(path, "w", encoding="utf-8", newline="") as f:
        f.write(text)


def make_icon(source: Path, log) -> None:
    """Writes `source` as the multi-size horizon.ico, on a square transparent canvas."""
    try:
        from PIL import Image
    except ImportError:
        raise RuntimeError("Pillow is missing: run  pip install pillow")
    if not source.is_file():
        raise RuntimeError(f"The icon picture {source} doesn't exist")
    image = Image.open(source).convert("RGBA")
    # A black background in the picture would make a black square icon: keep it as it is, but
    # centre the picture on a square so Windows doesn't stretch it.
    side = max(image.size)
    canvas = Image.new("RGBA", (side, side), image.getpixel((0, 0)))
    canvas.paste(image, ((side - image.width) // 2, (side - image.height) // 2))
    canvas = canvas.resize((256, 256), Image.LANCZOS)
    ICONS.mkdir(parents=True, exist_ok=True)
    backup = ICO.with_suffix(".ico.orig")
    if ICO.is_file() and not backup.exists():
        shutil.copy2(ICO, backup)
        log(f"Kept the original icon as {backup.name}")
    canvas.save(ICO, format="ICO", sizes=[(s, s) for s in SIZES])
    log(f"Icon written: {ICO.relative_to(ROOT)} ({len(SIZES)} sizes)")


# Edits that make the launcher POC404 and keep it as it is. Each is (file, old text, new text): done
# when `old` is there, left alone when `new` already is, so running them again changes nothing.
BRANDING = [
    ("launcher/src/titlebar.rs", 'title: Some("Horizon".into())', 'title: Some("POC404".into())'),
    (
        "launcher/src/titlebar.rs",
        'blocks::pixel(13., FontWeight::MEDIUM).child("Horizon")',
        'blocks::pixel(13., FontWeight::MEDIUM).child("POC404")',
    ),
    ("launcher/src/main.rs", '.child("Horizon"),\n', '.child("POC404"),\n'),
    # The launcher never downloads a newer launcher over itself: that would bring the old name,
    # icon and interface back. The game builds it installs (the backend) still update.
    (
        "launcher/src/backend.rs",
        "pub fn self_update_allowed() -> bool {\n"
        "    !cfg!(debug_assertions) && std::env::var_os(\"HORIZON_LAUNCHER_NO_SELF_UPDATE\").is_none()\n}",
        "pub fn self_update_allowed() -> bool {\n    false\n}",
    ),
    (
        "launcher/build.rs",
        "        resource.set_icon(&ico);\n",
        "        resource.set_icon(&ico);\n"
        "        for key in [\n"
        '            "ProductName",\n'
        '            "FileDescription",\n'
        '            "InternalName",\n'
        '            "CompanyName",\n'
        "        ] {\n"
        '            resource.set(key, "POC404");\n'
        "        }\n",
    ),
    # The memory slider's module, and the cap that keeps a game from freezing the computer.
    ("launcher/src/lib.rs", "pub mod loaders;\n", "pub mod loaders;\npub mod memory;\n"),
]

# What an update never replaces: the interface and the icon. Everything else is the backend.
PROTECTED = [
    "build.py",
    "packaging/icons/",
    "launcher/build.rs",
    "launcher/lang/",
    "launcher/assets/",
    "launcher/src/main.rs",
    "launcher/src/titlebar.rs",
    "launcher/src/theme.rs",
    "launcher/src/blocks.rs",
    "launcher/src/shell.rs",
    "launcher/src/memory_page.rs",
    "launcher/src/memory.rs",
    "launcher/src/sky.rs",
    "launcher/src/instance_icons.rs",
    "launcher/src/home.rs",
    "launcher/src/home_tabs.rs",
    "launcher/src/new_instance.rs",
    "launcher/src/version_choose.rs",
    "launcher/src/java_launch.rs",
]
PROTECTED_PATTERNS = ["launcher/src/*_page.rs"]


def _brand_ok(s: str) -> bool:
    if 'Horizon' not in s:
        return False
    if any(b in s for b in ['/','.app','.dev','http','Horizon.','\\\\']): return False
    if s.strip()=='Horizon Official': return False
    return True
def _brand_rewrite(t: str):
    """Replaces Horizon in ordinary string literals only; comments, chars and raw strings stay."""
    out=[];i=0;n=len(t);count=0
    while i<n:
        c=t[i]
        if t.startswith('//',i):
            j=t.find('\n',i); j=n if j<0 else j
            out.append(t[i:j]); i=j
        elif t.startswith('/*',i):
            depth=1;j=i+2
            while j<n and depth:
                if t.startswith('/*',j): depth+=1;j+=2
                elif t.startswith('*/',j): depth-=1;j+=2
                else: j+=1
            out.append(t[i:j]); i=j
        elif c=='r' and re.match(r'r#*"',t[i:i+8]) and (i==0 or not (t[i-1].isalnum() or t[i-1]=='_')):
            m=re.match(r'r(#*)"',t[i:]); h=m.group(1)
            end=t.find('"'+h,i+len(m.group(0)))
            j=n if end<0 else end+1+len(h)
            out.append(t[i:j]); i=j
        elif c=="'":
            m=re.match(r"'(?:\\.[^']*|[^\\'])'",t[i:])
            if m: out.append(m.group(0)); i+=len(m.group(0))
            else: out.append(c); i+=1      # a lifetime
        elif c=='"':
            j=i+1
            while j<n and t[j]!='"':
                j+=2 if t[j]=='\\' else 1
            lit=t[i:j+1]
            plain=lit[1:-1].replace('\\"','"')
            if _brand_ok(plain): lit=lit.replace('Horizon','POC404'); count+=1
            out.append(lit); i=j+1
        else:
            out.append(c); i+=1
    return ''.join(out),count

def rename_text(log) -> None:
    """Horizon -> POC404 in the text the launcher shows (string literals and the language tables),
    wherever an update brought the old name back. Paths, URLs, file names and comments stay."""
    import json

    count = 0
    files = list((LAUNCHER / "src").glob("**/*.rs")) + list((LAUNCHER / "tests").glob("**/*.rs"))
    for path in files:
        if path.name in ("mcp.rs", "game_app.rs"):
            continue
        text = read_text(path)
        new, n = _brand_rewrite(text)
        if n:
            write_text(path, new)
            count += n
    for table in (LAUNCHER / "lang").glob("*.json"):
        original = read_text(table)
        data = json.loads(original)
        out = {}
        for key, value in data.items():
            if _brand_ok(key):
                key, value = key.replace("Horizon", "POC404"), value.replace("Horizon", "POC404")
            out[key] = value
        new = json.dumps(out, ensure_ascii=False, indent=2) + "\n"
        if new != original.replace("\r\n", "\n"):
            write_text(table, new)
    if count:
        log(f"Renamed Horizon to POC404 in {count} texts")


def apply_branding(log) -> None:
    """Puts the POC404 name and the no-self-update rule back wherever an update removed them."""
    for name, old, new in BRANDING:
        path = ROOT / name
        if not path.is_file():
            log(f"Branding: {name} isn't here, skipped")
            continue
        text = read_text(path)
        if new in text:
            continue  # already done
        if old in text:
            write_text(path, text.replace(old, new))
            log(f"Branding: {name} updated")
        else:
            log(f"Branding: {name} changed upstream, so {old[:40]!r} wasn't found: check it by hand")
    rename_text(log)


def is_protected(relative: str) -> bool:
    from fnmatch import fnmatch

    return any(relative.startswith(p) if p.endswith("/") else relative == p for p in PROTECTED) or any(
        fnmatch(relative, p) for p in PROTECTED_PATTERNS
    )


def apply_update(source: Path, log) -> None:
    """Takes a newer project (a .zip, or its folder) in: the backend replaces ours, the interface
    and the icon stay. Files that exist only in the newer project are added (new backend)."""
    import tempfile
    import zipfile

    with tempfile.TemporaryDirectory() as scratch:
        folder = Path(source)
        if folder.is_file():
            with zipfile.ZipFile(folder) as z:
                z.extractall(scratch)
            folder = Path(scratch)
        # A zip holds one folder: step into it.
        while True:
            entries = [e for e in folder.iterdir() if e.name != "__MACOSX"]
            if len(entries) == 1 and entries[0].is_dir() and not (folder / "launcher").is_dir():
                folder = entries[0]
            else:
                break
        if not (folder / "launcher").is_dir():
            raise RuntimeError(f"{source} doesn't look like the project: it has no launcher folder")
        copied = kept = 0
        for path in folder.rglob("*"):
            if not path.is_file() or ".git" in path.parts or "target" in path.parts:
                continue
            relative = path.relative_to(folder).as_posix()
            if is_protected(relative) and (ROOT / relative).exists():
                kept += 1
                continue
            target = ROOT / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, target)
            copied += 1
    log(f"Update taken in: {copied} backend files copied, {kept} interface and icon files kept.")
    apply_branding(log)


class Build:
    """One build; `stop()` ends it."""

    def __init__(self, source: Path, out_dir: Path, log):
        self.source, self.out_dir, self.log = source, out_dir, log
        self.process: subprocess.Popen | None = None
        self.stopped = False

    def stop(self) -> None:
        self.stopped = True
        if self.process and self.process.poll() is None:
            self.process.terminate()

    def _cargo(self, command: list[str]) -> tuple[int, str]:
        """Runs a cargo command in launcher/, logging its output; returns the exit code and text."""
        self.process = subprocess.Popen(
            command,
            cwd=LAUNCHER,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            encoding="utf-8",
            errors="replace",
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        )
        assert self.process.stdout is not None
        lines = []
        for line in self.process.stdout:
            lines.append(line)
            self.log(line.rstrip())
        return self.process.wait(), "".join(lines)

    def run(self) -> Path:
        log = self.log
        if not IS_WINDOWS:
            log("Note: this is not Windows, so the result is not a .exe. Run this on Windows.")
        if shutil.which("cargo") is None:
            raise RuntimeError("Rust's cargo isn't installed: https://rustup.rs")
        apply_branding(log)
        make_icon(self.source, log)
        # build.rs only re-embeds when the .ico changes; make sure it runs again.
        (LAUNCHER / "build.rs").touch()
        log("Building (the first time takes a while)...")
        code, output = self._cargo(["cargo", "build", "--release"])
        # A build that was cut short (closed window, power cut) can leave a broken file in
        # target/: rustc then says "corrupt metadata" / E0786. Cleaning it is the fix.
        broken = ("E0786", "corrupt metadata", "invalid metadata files", "failed to mmap", "unexpected end of file")
        if code != 0 and not self.stopped and any(m in output for m in broken):
            log("The build cache is damaged (a build was cut short). Cleaning it and building again...")
            self._cargo(["cargo", "clean"])
            code, output = self._cargo(["cargo", "build", "--release"])
        if self.stopped:
            raise RuntimeError("Stopped.")
        if code != 0:
            raise RuntimeError(f"cargo failed (exit code {code}). The lines above say why.")
        built = LAUNCHER / "target" / "release" / EXE
        if not built.is_file():
            raise RuntimeError(f"The build finished but {built} isn't there")
        self.out_dir.mkdir(parents=True, exist_ok=True)
        target = self.out_dir / (f"{NAME}.exe" if IS_WINDOWS else NAME)
        shutil.copy2(built, target)
        log(f"Done: {target}")
        return target


def run_gui() -> None:
    import tkinter as tk
    from tkinter import filedialog, messagebox, ttk

    BG, PANEL, FIELD = "#000000", "#000000", "#0d0d0d"
    INK, MUTED, ACCENT, RULE = "#ffffff", "#9a9a9a", "#ffffff", "#2a2a2a"
    DANGER, OK = "#ff6b6b", "#5fd38d"

    root = tk.Tk()
    root.title("POC404: build the .exe")
    root.geometry("860x600")
    root.minsize(640, 460)
    root.configure(bg=BG)
    try:
        # The window takes the same picture as its icon.
        from PIL import Image, ImageTk

        if DEFAULT_SOURCE.is_file():
            root._icon = ImageTk.PhotoImage(Image.open(DEFAULT_SOURCE).convert("RGBA"))
            root.iconphoto(True, root._icon)
    except Exception:
        pass

    style = ttk.Style(root)
    style.theme_use("clam")
    style.configure(".", background=BG, foreground=INK, fieldbackground=FIELD, bordercolor=RULE)
    style.configure("TFrame", background=BG)
    style.configure("TLabel", background=BG, foreground=INK)
    style.configure("Muted.TLabel", foreground=MUTED)
    style.configure("Title.TLabel", font=("Segoe UI", 18, "bold"))
    style.configure("TEntry", fieldbackground=FIELD, foreground=INK, insertcolor=INK, bordercolor=RULE)
    # Buttons are white with black text; grey when they can't be pressed.
    style.configure("TButton", background="#ffffff", foreground="#000000", bordercolor="#ffffff",
                    focuscolor="#ffffff", padding=(14, 8))
    style.map("TButton", background=[("active", "#d9d9d9"), ("disabled", "#3a3a3a")],
              foreground=[("disabled", "#8a8a8a")])
    style.configure("Accent.TButton", font=("Segoe UI", 10, "bold"))
    style.configure("Horizontal.TProgressbar", troughcolor=PANEL, background=ACCENT, bordercolor=RULE)

    frame = ttk.Frame(root, padding=20)
    frame.pack(fill="both", expand=True)
    ttk.Label(frame, text="Build POC404 as an .exe", style="Title.TLabel").pack(anchor="w")
    ttk.Label(
        frame,
        text="Your picture becomes the program's icon, then the launcher is built.",
        style="Muted.TLabel",
    ).pack(anchor="w", pady=(2, 16))

    source_var = tk.StringVar(value=str(DEFAULT_SOURCE))
    out_var = tk.StringVar(value=str(ROOT / "dist"))

    def path_row(label: str, var: tk.StringVar, browse) -> None:
        row = ttk.Frame(frame)
        row.pack(fill="x", pady=4)
        ttk.Label(row, text=label, width=12).pack(side="left")
        ttk.Entry(row, textvariable=var).pack(side="left", fill="x", expand=True, padx=8)
        ttk.Button(row, text="Browse…", command=browse).pack(side="left")

    def browse_icon() -> None:
        chosen = filedialog.askopenfilename(
            title="Icon picture", filetypes=[("Pictures", "*.png *.jpg *.jpeg *.bmp *.ico")]
        )
        if chosen:
            source_var.set(chosen)

    def browse_out() -> None:
        chosen = filedialog.askdirectory(title="Where to put the .exe")
        if chosen:
            out_var.set(chosen)

    path_row("Icon picture", source_var, browse_icon)
    path_row("Output folder", out_var, browse_out)

    buttons = ttk.Frame(frame)
    buttons.pack(fill="x", pady=(14, 8))
    build_button = ttk.Button(buttons, text="Build .exe", style="Accent.TButton")
    build_button.pack(side="left")
    stop_button = ttk.Button(buttons, text="Stop", state="disabled")
    stop_button.pack(side="left", padx=8)
    update_button = ttk.Button(buttons, text="Take in an update…")
    update_button.pack(side="left", padx=(8, 0))
    open_button = ttk.Button(buttons, text="Open output folder", state="disabled")
    open_button.pack(side="left")
    status = ttk.Label(buttons, text="Ready", style="Muted.TLabel")
    status.pack(side="right")

    progress = ttk.Progressbar(frame, mode="indeterminate")
    progress.pack(fill="x", pady=(0, 8))

    log_box = tk.Text(
        frame, bg=PANEL, fg=INK, insertbackground=INK, relief="flat", wrap="none",
        font=("Consolas", 10), highlightthickness=1, highlightbackground=RULE, state="disabled",
    )
    log_box.pack(fill="both", expand=True)
    log_box.tag_configure("error", foreground=DANGER)
    log_box.tag_configure("ok", foreground=OK)

    lines: queue.Queue[str] = queue.Queue()
    state: dict = {"build": None}

    def pump() -> None:
        try:
            while True:
                line = lines.get_nowait()
                tag = "error" if line.startswith(("error", "Error")) else "ok" if line.startswith("Done") else ""
                log_box.configure(state="normal")
                log_box.insert("end", line + "\n", tag)
                log_box.see("end")
                log_box.configure(state="disabled")
        except queue.Empty:
            pass
        root.after(80, pump)

    def finished(ok: bool, message: str) -> None:
        progress.stop()
        build_button.configure(state="normal")
        stop_button.configure(state="disabled")
        status.configure(text="Done" if ok else "Failed", foreground=OK if ok else DANGER)
        if ok:
            open_button.configure(state="normal")
        else:
            messagebox.showerror("Build failed", message)

    def start() -> None:
        build = Build(Path(source_var.get()), Path(out_var.get()), lines.put)
        state["build"] = build
        build_button.configure(state="disabled")
        stop_button.configure(state="normal")
        status.configure(text="Building…", foreground=MUTED)
        progress.start(12)

        def work() -> None:
            try:
                build.run()
                root.after(0, finished, True, "")
            except Exception as e:  # shown in the window
                lines.put(f"Error: {e}")
                root.after(0, finished, False, str(e))

        threading.Thread(target=work, daemon=True).start()

    def stop() -> None:
        if state["build"]:
            state["build"].stop()

    def take_update() -> None:
        chosen = filedialog.askopenfilename(title="Newer project (.zip)", filetypes=[("Zip", "*.zip")])
        if not chosen:
            return
        try:
            apply_update(Path(chosen), lines.put)
        except Exception as e:
            lines.put(f"Error: {e}")
            messagebox.showerror("Update failed", str(e))

    def open_out() -> None:
        folder = out_var.get()
        if IS_WINDOWS:
            os.startfile(folder)  # type: ignore[attr-defined]
        else:
            subprocess.Popen(["xdg-open", folder])

    build_button.configure(command=start)
    stop_button.configure(command=stop)
    update_button.configure(command=take_update)
    open_button.configure(command=open_out)
    pump()
    root.mainloop()


def main() -> int:
    if "--apply-update" in sys.argv:
        try:
            apply_update(Path(sys.argv[sys.argv.index("--apply-update") + 1]), print)
        except (IndexError, RuntimeError, OSError) as e:
            print(f"Error: {e}")
            return 1
        return 0
    if "--build" in sys.argv:
        try:
            Build(DEFAULT_SOURCE, ROOT / "dist", print).run()
        except RuntimeError as e:
            print(f"Error: {e}")
            return 1
        return 0
    run_gui()
    return 0


if __name__ == "__main__":
    sys.exit(main())
