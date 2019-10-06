import json
import os
from threading import Lock, Condition


class Store:
    # TODO allow env var override
    PATH = os.path.join(os.path.abspath(os.path.dirname(__file__)), "tweaker.json")

    def __init__(self, **kwargs):
        self._dict = dict(**kwargs)
        self.callbacks = []

        self.mutex = Lock()
        self.cond = Condition(self.mutex)
        self.changed = None
        self.callbacks_enabled = False

    @classmethod
    def load(cls):
        try:
            with open(cls.PATH) as f:
                store = json.load(f)
                print(f"loaded {len(store)} entries")
        except FileNotFoundError:
            print("made new store")
            store = {}
        return cls(**store)

    def save(self):
        with self.mutex:
            with open(self.PATH, "w") as f:
                json.dump(self._dict, f, indent=4)
                print(f"saved {len(self._dict)} entries")

    def __setitem__(self, key, value):
        with self.mutex:
            me = self._dict[key]
            my_type = type(me["value"])

            # terrible edge case with bool("anything") == true
            if my_type == bool:
                value = value == "True"
            else:
                value = my_type(value)
            me["value"] = value

            if self.callbacks_enabled:
                self.changed = (key, value)
                self.cond.notify_all()

    def take_changed(self):
        changed = self.changed
        self.changed = None
        return changed

    def enable_callbacks(self):
        self.callbacks_enabled = True

    def register(self, name, initial, increment):
        if name in self._dict:
            raise RuntimeError("'{}' already exists".format(name))

        self._dict[name] = {
            "value": initial,
        }
        if increment is not None:
            self._dict[name]["increment"] = increment

    def fields(self):
        return ((k, v["value"], v.get("increment")) for (k, v) in self._dict.items())

    def items(self):
        return {k: v["value"] for (k, v) in self._dict.items()}
