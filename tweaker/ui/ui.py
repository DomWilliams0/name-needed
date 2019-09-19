import atexit
import os

import urwid

import server
from store import Store


class Value:
    def __init__(self, store, key, val, increment):
        self.key = key
        self.val = val
        self.incr = increment
        self.txt = urwid.Text("", align="center")

        self.callbacks = [
            lambda s: self.txt.set_text(("value", s)),
            lambda s: store.__setitem__(key, s),
        ]
        self.update()

    def update(self):
        val = str(self)
        for cb in self.callbacks:
            cb(val)

    def on_add(self):
        self.val += self.incr
        self.update()

    def on_sub(self):
        self.val -= self.incr
        self.update()

    def __str__(self):
        return str(self.val)[:8]


def add_field(value):
    def button(txt, func):
        b = (1, urwid.Button(("button", txt), on_press=lambda _: func()))
        b[1]._label.align = "center"
        return b

    cols = urwid.Columns([
        urwid.Padding(urwid.Text(("name", value.key), align=urwid.RIGHT), right=4),
        button("+", value.on_add),
        value.txt,
        button("-", value.on_sub),
    ])

    return cols


def mk_window(store):
    fields = [add_field(Value(store, *field)) for field in store.fields()]

    pile = urwid.Pile(fields)
    widget = urwid.Filler(pile, 'top')
    palette = [
        ("name", "light blue", "default"),
        ("value", "dark green", "default"),
        ("button", "light gray", "default"),
    ]

    def showorexit(key):
        if key in ('q', 'Q', 'esc'):
            raise urwid.ExitMainLoop()

    loop = urwid.MainLoop(widget, palette, unhandled_input=showorexit)

    store.enable_callbacks()
    loop.run()


if __name__ == '__main__':
    store = Store.load()
    atexit.register(Store.save, store)

    thread = server.start_server(store)
    mk_window(store)
    os._exit(0)  # please just exit
