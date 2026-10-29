#!/usr/bin/python3
"""Native Linux IBus client. Requires live ibus-daemon and real engine.
stdout: READY / P<TAB>utf8_hex<TAB>unicode_scalar_caret / C<TAB>utf8_hex / H / FAIL.
stdin: K<TAB>keysym<TAB>keycode<TAB>state, F<TAB>0|1, Q.
--probe sends real IBus ProcessKeyEvent to installed Hangul engine.
"""
import os
import sys
try:
    import gi
    gi.require_version("IBus", "1.0")
    from gi.repository import IBus, GLib
except Exception as exc:
    print("IBUS_GI_UNAVAILABLE:" + repr(exc), file=sys.stderr, flush=True)
    sys.exit(2)
IBus.init()
bus = IBus.Bus()
if not bus.is_connected():
    print("IBUS_DAEMON_NOT_CONNECTED", file=sys.stderr, flush=True)
    sys.exit(3)
context = bus.create_input_context("aura-lumina-native")
if context is None:
    print("IBUS_NO_INPUT_CONTEXT", file=sys.stderr, flush=True)
    sys.exit(4)
context.set_capabilities(int(IBus.Capabilite.FOCUS) | int(IBus.Capabilite.PREEDIT_TEXT))
engine = os.environ.get("AURA_LUMINA_IBUS_ENGINE", "")
if engine:
    # Global-engine mode normally overrides an individual InputContext's
    # requested engine on FocusIn; select the server-side global engine too.
    if not bus.set_global_engine(engine):
        print("IBUS_GLOBAL_ENGINE_REJECTED:"+engine, file=sys.stderr, flush=True)
        sys.exit(5)
    context.set_engine(engine)
context.focus_in()
stats = {"preedit": 0, "commit": 0, "unicode": False}
def emit(line):
    print(line, flush=True)
def valid_text(value):
    text = value.get_text()
    if not isinstance(text, str) or len(text.encode("utf-8")) > 1024:
        raise ValueError("unbounded IBus text")
    if any(ord(c) < 32 or ord(c) == 127 for c in text):
        raise ValueError("control in IBus text")
    return text
def preedit(_ctx, value, cursor, visible):
    try:
        text = valid_text(value)
        if not visible or not text:
            emit("H")
            return
        if int(cursor) > len(text):
            raise ValueError("cursor past text")
        stats["preedit"] += 1
        stats["unicode"] |= any(ord(c) > 127 for c in text)
        emit("P\t" + text.encode().hex() + "\t" + str(cursor))
    except Exception as exc:
        print("BAD_PREEDIT:" + repr(exc), file=sys.stderr, flush=True)
        emit("FAIL")
def commit(_ctx, value):
    try:
        text = valid_text(value)
        if text:
            stats["commit"] += 1
            stats["unicode"] |= any(ord(c) > 127 for c in text)
            emit("C\t" + text.encode().hex())
    except Exception as exc:
        print("BAD_COMMIT:" + repr(exc), file=sys.stderr, flush=True)
        emit("FAIL")
context.connect("update-preedit-text", preedit)
context.connect("commit-text", commit)
context.connect("hide-preedit-text", lambda _: emit("H"))
loop = GLib.MainLoop()
readiness={"attempts":0, "ready":False}
def wait_for_selected_engine():
    readiness["attempts"] += 1
    selected=context.get_engine()
    name=selected.get_name() if selected else "NONE"
    if not engine or name==engine:
        readiness["ready"]=True
        emit("READY")
        print("IBUS_ACTUAL_ENGINE_READY:"+name, file=sys.stderr, flush=True)
        return False
    if readiness["attempts"]>=60:
        print("IBUS_ENGINE_MISMATCH requested="+engine+" actual="+name,
              file=sys.stderr, flush=True)
        emit("FAIL")
        loop.quit()
        return False
    if readiness["attempts"]%10==0:
        bus.set_global_engine(engine)
        context.set_engine(engine)
        context.focus_in()
    return True
GLib.timeout_add(80, wait_for_selected_engine)
def key(keysym, keycode=0, state=0):
    consumed=bool(context.process_key_event(keysym, keycode, state))
    if "--probe" in sys.argv:
        print("IBUS_PROBE_KEY",hex(keysym),"handled",consumed,
              file=sys.stderr,flush=True)
    return consumed
def read_stdin(_fd, condition):
    if condition & (GLib.IO_HUP | GLib.IO_ERR):
        loop.quit()
        return False
    raw = sys.stdin.buffer.readline(129)
    if not raw or len(raw) > 128 or not raw.endswith(b"\n"):
        loop.quit()
        return False
    try:
        pieces = raw.decode("ascii").strip().split("\t")
        if pieces == ["Q"]:
            loop.quit()
            return False
        if len(pieces) == 2 and pieces[0] == "F" and pieces[1] in ("0", "1"):
            (context.focus_in if pieces[1] == "1" else context.focus_out)()
            if pieces[1] == "0":
                emit("H")
            emit("A\t" + pieces[1])
            return True
        if len(pieces) == 4 and pieces[0] == "K":
            args = [int(v) for v in pieces[1:]]
            if any(v < 0 or v > 0xFFFFFFFF for v in args):
                raise ValueError("unbounded key")
            key(*args)
            return True
        raise ValueError("unknown command")
    except Exception as exc:
        print("IBUS_BAD_COMMAND:" + repr(exc), file=sys.stderr, flush=True)
        emit("FAIL")
        loop.quit()
        return False
if "--probe" in sys.argv:
    if engine != "hangul":
        print("PROBE_REQUIRES_HANGUL", file=sys.stderr, flush=True)
        sys.exit(5)
    # Actual two-set Hangul keyboard: r,k combine to Korean syllable.
    # First inspect whether the context really owns a selected engine.
    def report_engine():
        e=context.get_engine()
        print("IBUS_PROBE_ENGINE",e.get_name() if e else "NONE",
              file=sys.stderr,flush=True)
        return False
    GLib.timeout_add(1000,report_engine)
    steps = iter([ord("r"), ord("k"), ord(" "), 0xFF0D])
    def next_key():
        try:
            key(next(steps))
            return True
        except StopIteration:
            return False
    def begin():
        GLib.timeout_add(120, next_key)
        return False
    def verify():
        ok = stats["preedit"] > 0 and stats["commit"] > 0 and stats["unicode"]
        if ok:
            emit("IBUS_REAL_ENGINE_PROBE_PASS\t" + str(stats["preedit"]) + "\t" + str(stats["commit"]))
        else:
            print("IBUS_REAL_ENGINE_PROBE_FAIL:" + repr(stats), file=sys.stderr, flush=True)
        loop.quit()
        return False
    GLib.timeout_add(2200, begin)
    GLib.timeout_add(7000, verify)
    loop.run()
    sys.exit(0 if stats["preedit"] > 0 and stats["commit"] > 0 and stats["unicode"] else 6)
GLib.io_add_watch(sys.stdin.fileno(), GLib.IO_IN | GLib.IO_HUP | GLib.IO_ERR, read_stdin)
loop.run()
