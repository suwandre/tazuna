"""Inside the Linux bench jail (bubblewrap, its own network namespace with only loopback): 127.0.0.1:PORT forwards to
the bench's blocking proxy through a Unix socket bound into the jail, then the agent's command runs. The agent keeps
HTTPS_PROXY=http://127.0.0.1:PORT as on the Mac, and nothing else on the network is reachable.

    netbridge.py PORT SOCKET -- COMMAND ...
"""
import os, socket, sys, threading

port, path, cmd = int(sys.argv[1]), sys.argv[2], sys.argv[4:]


def pipe(a, b):
    try:
        while d := a.recv(65536):
            b.sendall(d)
    except OSError:
        pass
    finally:
        for s in (a, b):
            try:
                s.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass


def serve(srv):
    while True:
        c, _ = srv.accept()
        u = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            u.connect(path)
        except OSError:
            c.close()
            continue
        threading.Thread(target=pipe, args=(c, u), daemon=True).start()
        threading.Thread(target=pipe, args=(u, c), daemon=True).start()


srv = socket.create_server(("127.0.0.1", port))
if os.fork() == 0:  # the bridge; the jail's own PID namespace ends it with the command
    serve(srv)
    os._exit(0)
srv.close()
os.execvp(cmd[0], cmd)
