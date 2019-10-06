import json
import os
import socketserver
import struct
import sys
import threading
import traceback

PORT = 44448


class Protocol:
    @staticmethod
    def init(store):
        return Protocol.update(store.items())

    @staticmethod
    def update(items):
        return json.dumps(items).encode()


class Handler(socketserver.BaseRequestHandler):
    def handle(self):
        store = self.server.store

        # on connect: send full state
        self._send(Protocol.init(store))

        while True:
            with store.mutex:
                while True:
                    if store.changed is not None:
                        break
                    store.cond.wait()

                k, v = store.changed
                try:
                    self._send(Protocol.update({k: v}))

                    # only take if send was successful
                    store.take_changed()
                except BrokenPipeError:
                    # sys.stderr.write("socket ded\n")
                    return

    def _send(self, wot):
        l = struct.pack("@H", len(wot))  # u16
        self.request.sendall(l)
        self.request.sendall(wot)


def start_server(store):
    def func():
        server = socketserver.ThreadingTCPServer(("127.0.0.1", PORT), Handler, bind_and_activate=False)
        server.allow_reuse_address = True
        server.store = store

        try:
            server.server_bind()
            server.server_activate()
            server.serve_forever()
        except:
            traceback.print_exc()
            os._exit(1)  # actually exit

    t = threading.Thread(target=func)
    t.start()
    return t
