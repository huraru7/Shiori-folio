# 今の待機の絵(idle_base.png)から、点を最小限だけ動かして作る短い動き: 反応する・着地・喜ぶ・現れる
# - 沈む: 呼吸と同じく、継ぎ目(縦92、ひだが縦に通る無地のローブの行)より上を、1〜2ドット下げる
# - 跳ねる: 絵の全体を上へずらす(頭の上には11行の余白がある)
# - 目: 閉じ目・半目は待機と同じ。喜ぶの目は、上まぶたを1段下げたやわらかい目
# - 現れる: ランプの灯り → ランプ → ランプから近い順(少しばらつかせる)に点が現れる。半透明の点は作らない
# コマ数は brain.rs の状態の長さ×8(反応0.4秒=4、着地0.5秒=4、喜ぶ2秒=16、現れる1秒=8)
# 使い方(このフォルダで): python make_simple.py idle_base.png <出力のフォルダ>
import sys
import numpy as np
from PIL import Image

from make_base import PAL

P = dict(PAL)
P['eyeL'] = (180, 232, 212)
P['faceLine'] = (118, 96, 96)
INV = {v: k for k, v in P.items()}

base = np.array(Image.open(sys.argv[1]).convert('RGBA'))
out_dir = sys.argv[2]


def nm(a, x, y):
    if a[y, x, 3] == 0:
        return None
    return INV.get(tuple(int(v) for v in a[y, x, :3]), '?')


def put(a, x, y, n):
    a[y, x, :3] = P[n]
    a[y, x, 3] = 255


SEAM = 92


def sink(a, k):
    """継ぎ目より上を k ドット下げる"""
    if k == 0:
        return a
    b = a.copy()
    b[k:SEAM + 1] = a[0:SEAM + 1 - k]
    return b


def hop(a, h):
    """絵の全体を h ドット上げる"""
    if h == 0:
        return a
    b = np.zeros_like(a)
    b[:192 - h] = a[h:]
    return b


EYES = [(54, 59, 'left'), (68, 73, 'right')]


def eyes_half(a):
    b = a.copy()
    for x0, x1, _ in EYES:
        for x in range(x0, x1 + 1):
            for y in (33, 34):
                put(b, x, y, 'skin')
            put(b, x, 35, 'line')
    return b


def eyes_closed(a):
    b = a.copy()
    for x0, x1, side in EYES:
        for x in range(x0, x1 + 1):
            for y in range(33, 38):
                put(b, x, y, 'skin')
        # 目尻(外側)だけ1段下げて、たれ目の形を残す
        if side == 'left':
            for x in range(x0 + 1, x1 + 1):
                put(b, x, 35, 'line')
            put(b, x0, 36, 'line')
        else:
            for x in range(x0, x1):
                put(b, x, 35, 'line')
            put(b, x1, 36, 'line')
    return b


def eyes_happy(a):
    """喜ぶの目: 上まぶたを1段下げた、やわらかい目。口は片側だけ1段上げて、穏やかに笑う
    (「^^」の目と両端を上げた口は、にっこりしすぎてキャラクターに合わなかった。ふらる)"""
    b = a.copy()
    for x0, x1, _ in EYES:
        for x in range(x0, x1 + 1):
            put(b, x, 33, 'skin')
            put(b, x, 34, 'line')
    put(b, 65, 42, 'mouth')
    return b


# ランプのガラスの中(灯りの揺れ。待機と同じ範囲)
GLASS = (29, 38, 128, 137)


def lamp_bright(a):
    b = a.copy()
    x0, x1, y0, y1 = GLASS
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            if nm(a, x, y) == 'glow':
                put(b, x, y, 'glowL')
    return b


# --- 反応する(押された): 沈んで目をつむり、戻る。4コマ ---
react = [sink(eyes_closed(base), 1), sink(eyes_closed(base), 2), sink(eyes_half(base), 1), base]

# --- 着地: 深く沈んで、ゆっくり戻る。4コマ ---
land = [sink(eyes_closed(base), 2), sink(eyes_closed(base), 2), sink(eyes_half(base), 1), sink(base, 1)]

# --- 喜ぶ: やわらかい目で2回跳ねる。1回8コマ(かがむ → 上がる → 下りる)。灯りは明るい ---
happy_face = lamp_bright(eyes_happy(base))
HOP = [-1, 3, 6, 8, 8, 6, 3, 0]
happy = [sink(happy_face, 1) if h < 0 else hop(happy_face, h) for h in HOP] * 2

# --- 現れる: ランプの灯り → ランプ → ランプから近い順に点が現れる。8コマ ---
LAMP_C = {'brass', 'brassD', 'brassL', 'glow', 'glowL', 'gold'}
ys, xs = np.where(base[..., 3] > 0)
glass = np.zeros((192, 128), bool)
lamp = np.zeros((192, 128), bool)
for y, x in zip(ys, xs):
    n = nm(base, x, y)
    if x <= 42 and y >= 104 and n in LAMP_C:
        lamp[y, x] = True
        if n in ('glow', 'glowL'):
            glass[y, x] = True
rng = np.random.default_rng(7)
cx, cy = 33.5, 132.5
order = np.full((192, 128), np.inf)
for y, x in zip(ys, xs):
    if not lamp[y, x]:
        order[y, x] = np.hypot(x - cx, y - cy) + rng.uniform(0, 40)
finite = order[np.isfinite(order)]


def reveal(frac):
    a = np.zeros_like(base)
    show = lamp | (order <= np.quantile(finite, frac))
    a[show] = base[show]
    return a


only_glass = np.zeros_like(base)
only_glass[glass] = base[glass]
appear = [only_glass, reveal(0.0), reveal(0.2), reveal(0.45), reveal(0.7), reveal(0.9), base, base]

for name, seq in (('react', react), ('land', land), ('happy', happy), ('appear', appear)):
    Image.fromarray(np.concatenate(seq, axis=1), 'RGBA').save(f'{out_dir}/{name}.png')
    print(name, len(seq), 'コマ')
