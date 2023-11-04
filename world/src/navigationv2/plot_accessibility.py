import json

import matplotlib.patches
import matplotlib.pyplot as plt
import matplotlib.animation
import sys

input_json = sys.argv[1]

with open(input_json) as f:
    frames = json.load(f)

fig, ax = plt.subplots()
# ax.set_box_aspect(1)
# ax.axis("scaled")
ax.grid(alpha=0.3, which="both")
ax.xaxis.set_minor_locator(matplotlib.ticker.MultipleLocator(1))
ax.yaxis.set_minor_locator(matplotlib.ticker.MultipleLocator(1))
ax.set_aspect('equal', adjustable='box')

cur_lims = None


def set_lims(lims):
    margin = 2
    [nx1, nx2, ny1, ny2] = lims
    if nx1 is not None:
        nx1 -= margin
    if nx2 is not None:
        nx2 += margin
    if ny1 is not None:
        ny1 -= margin
    if ny2 is not None:
        ny2 += margin

    global cur_lims
    if cur_lims is not None:
        if nx1 is not None:
            cur_lims[0] = min(cur_lims[0], nx1)
        if nx2 is not None:
            cur_lims[1] = max(cur_lims[1], nx2)
        if ny1 is not None:
            cur_lims[2] = min(cur_lims[2], ny1)
        if ny2 is not None:
            cur_lims[3] = max(cur_lims[3], ny2)
    else:
        cur_lims = [nx1, nx2, ny1, ny2]

    [x1, x2, y1, y2] = cur_lims
    ax.set_xlim(x1, x2)
    ax.set_ylim(y1, y2)


def do_frame(frame):
    i, frame = frame
    ax.clear()

    all_points = [
        r[p]
        for p in ("min", "max")
        for x in ("agent_rects", "to_check")
        for r in frame[x]
    ]

    min_x = min((p[0] for p in all_points), default=None)
    min_y = min((p[1] for p in all_points), default=None)
    max_x = max((p[0] for p in all_points), default=None)
    max_y = max((p[1] for p in all_points), default=None)

    set_lims([min_x, max_x, min_y, max_y])

    def draw_rect(r, c):
        x1, y1 = r["min"]
        x2, y2 = r["max"]
        # y2 += 1
        # x2 += 1

        points = [
            (x1, y1),
            (x1, y2),
            (x2, y2),
            (x2, y1),
        ]
        ax.add_patch(matplotlib.patches.Polygon(points, linewidth=1, edgecolor="black", facecolor=c, alpha=0.5))

    for r in frame["agent_rects"]:
        draw_rect(r, "blue")

    for r in frame["to_check"]:
        draw_rect(r, "red")

    ax.set_title(f"{i}")


interval = 500
anim = matplotlib.animation.FuncAnimation(fig, do_frame, enumerate(frames), interval=interval)

paused = False


def toggle_play(evt):
    if evt.key != " ":
        return

    global paused
    paused = not paused
    if paused:
        anim.event_source.stop()
    else:
        anim.event_source.start()


fig.canvas.mpl_connect("key_press_event", toggle_play)

plt.show()
