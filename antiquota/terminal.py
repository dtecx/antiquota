"""A shell per session for the Terminal panel: a pseudo-terminal on POSIX, ConPTY on Windows.

Output is kept in a bounded buffer, so a panel that opens later (or reconnects) sees the
recent screen, and is pushed to every subscriber as raw bytes.
"""
import os
import queue
import subprocess
import threading

KEEP = 256 * 1024  # bytes of recent output a new subscriber gets


class Terminal:
    def __init__(self, cwd, cols=100, rows=30):
        self.subs, self.buf, self.lock, self.alive = [], bytearray(), threading.Lock(), True
        self.cwd = cwd
        env = {**os.environ, "TERM": "xterm-256color", "COLORTERM": "truecolor"}
        if os.name == "nt":
            from winpty import PtyProcess  # pywinpty, bundled with the Windows engine
            shell = os.environ.get("COMSPEC") or "powershell.exe"
            self.win = PtyProcess.spawn(shell, cwd=cwd, env=env, dimensions=(rows, cols))
            self.shell = os.path.basename(shell)
            threading.Thread(target=self._read_windows, daemon=True).start()
        else:
            import pty
            shell = os.environ.get("SHELL") or "/bin/zsh"
            master, slave = pty.openpty()
            self._winsize(master, cols, rows)
            self.proc = subprocess.Popen([shell, "-l"], stdin=slave, stdout=slave, stderr=slave, cwd=cwd, env=env,
                                         start_new_session=True, close_fds=True)
            os.close(slave)
            self.fd, self.shell = master, os.path.basename(shell)
            threading.Thread(target=self._read_posix, daemon=True).start()

    @staticmethod
    def _winsize(fd, cols, rows):
        import fcntl
        import struct
        import termios
        fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", max(2, int(rows)), max(2, int(cols)), 0, 0))

    def _read_posix(self):
        while True:
            try:
                data = os.read(self.fd, 65536)
            except OSError:
                break
            if not data:
                break
            self._emit(data)
        self._ended()

    def _read_windows(self):
        while True:
            try:
                data = self.win.read(4096)
            except EOFError:
                break
            self._emit(data.encode("utf-8", "replace"))
        self._ended()

    def _ended(self):
        self.alive = False
        self._emit(b"\r\n\x1b[2m[process exited]\x1b[0m\r\n")
        with self.lock:
            for q in self.subs:
                try:
                    q.put_nowait(None)
                except queue.Full:
                    pass

    def _emit(self, data):
        with self.lock:
            self.buf += data
            if len(self.buf) > KEEP:
                del self.buf[:len(self.buf) - KEEP]
            for q in list(self.subs):
                try:
                    q.put_nowait(data)
                except queue.Full:
                    pass

    def subscribe(self):
        """(recent output, queue of new chunks; None means the shell exited)."""
        q = queue.Queue(maxsize=4000)
        with self.lock:
            self.subs.append(q)
            return bytes(self.buf), q

    def unsubscribe(self, q):
        with self.lock:
            if q in self.subs:
                self.subs.remove(q)

    def write(self, text):
        if not self.alive:
            raise ValueError("the shell has exited: restart it")
        if os.name == "nt":
            self.win.write(text)
        else:
            os.write(self.fd, text.encode())

    def resize(self, cols, rows):
        if not self.alive:
            return
        if os.name == "nt":
            self.win.setwinsize(int(rows), int(cols))
        else:
            self._winsize(self.fd, cols, rows)

    def kill(self):
        try:
            if os.name == "nt":
                self.win.terminate(force=True)
            else:
                import signal
                os.killpg(self.proc.pid, signal.SIGHUP)
        except (OSError, AttributeError):
            pass
