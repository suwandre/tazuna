"""Inside the Linux bench jail (bubblewrap, its own network namespace with only loopback): 127.0.0.1:PORT forwards to
the bench's blocking proxy through a Unix socket bound into the jail, then the agent's command runs. The agent keeps
HTTPS_PROXY=http://127.0.0.1:PORT as on the Mac, and nothing else on the network is reachable. More PORT SOCKET pairs
bridge the run's relays the same way (their localhost URLs, as harnesses on the Mac reach them; 2026-11-09).

    netbridge.py PORT SOCKET [PORT SOCKET ...] -- COMMAND ...
"""
import os, socket, sys, threading

end = sys.argv.index("--")
pairs, cmd = [(int(sys.argv[i]), sys.argv[i + 1]) for i in range(1, end, 2)], sys.argv[end + 1:]


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


def serve(srv, path):
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


srvs = [(socket.create_server(("127.0.0.1", port)), path) for port, path in pairs]
if os.fork() == 0:  # the bridge; the jail's own PID namespace ends it with the command
    for srv, path in srvs[1:]:
        threading.Thread(target=serve, args=(srv, path), daemon=True).start()
    serve(*srvs[0])
    os._exit(0)
for srv, _ in srvs:
    srv.close()
os.execvp(cmd[0], cmd)
