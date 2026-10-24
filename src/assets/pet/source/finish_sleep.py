# 眠るの下絵(sleep_raw.png、4つの姿)を仕上げる。直すのはランプの所と、寝息・灯りの動きだけ
# - ランプは、4番の姿のものを、4つの姿の同じ位置に置く(姿ごとのランプのずれをなくす)
# - かぶる(hood): 4つの姿を2コマずつ(8コマ・1秒)。コードが「始め+繰り返し」に対応したら使う
# - 眠る(sleep): 4番の姿で、寝息(上半身を1ドット下げる)と、灯りの弱い明滅。32コマ・4秒で1周
# - 起きる(wake): かぶるの逆順(4番→1番)を1コマずつと、最後に1番をもう1コマ(5コマ。コードの0.6秒に合わせた)
# 使い方(このフォルダで): python finish_sleep.py sleep_raw.png hood.png ../sleep.png ../wake.png
import sys
import numpy as np
from PIL import Image

from make_base import PAL

P = dict(PAL)
P['eyeL'] = (180, 232, 212)
P['faceLine'] = (118, 96, 96)
P['page'] = (236, 226, 200)
INV = {v: k for k, v in P.items()}
LAMP = {'brassD', 'brass', 'brassL', 'glow', 'glowL', 'gold', 'bootD', 'boot', 'bootL', 'page', 'skin', 'skinD', 'star', 'mouth'}
EDGE = {'faceLine', 'hairD', 'line'}   # ランプの縁に使われている色

raw = np.array(Image.open(sys.argv[1]).convert('RGBA'))
fr = [raw[:, i * 128:(i + 1) * 128].copy() for i in range(raw.shape[1] // 128)]


def nm(a, x, y):
    if not (0 <= x < 128 and 0 <= y < 192) or a[y, x, 3] == 0:
        return None
    return INV.get(tuple(int(v) for v in a[y, x, :3]), '?')


def put(a, x, y, n):
    if n is None:
        a[y, x] = 0
    else:
        a[y, x, :3] = P[n]
        a[y, x, 3] = 255


def lamp_rows(a, y0, y1, x1):
    """その行の、ランプの色の点の左端から右端まで(横x1より左)"""
    rows = {}
    for y in range(y0, y1):
        xs = [x for x in range(0, x1) if nm(a, x, y) in LAMP]
        if xs:
            rows[y] = (xs[0], xs[-1])
    return rows


# 4番の姿のランプ(縦138〜171、横39より左)を切り出す。ガラスの中(156行目から)の透明な穴は明るい灯りで埋める
LAMP_Y = (138, 172)
lamp_px = {}
for y, (x0, x1) in lamp_rows(fr[3], *LAMP_Y, 39).items():
    for x in range(x0 - 1, x1 + 2):
        n = nm(fr[3], x, y)
        if n in ('robe', 'robeD', 'robeL'):
            continue
        if n is None:
            if y >= 156 and x0 < x < x1:
                n = 'glowL'
            elif x in (x0 - 1, x1 + 1):
                continue
        if n:
            lamp_px[(x, y)] = n


def unify_lamp(a):
    """その姿のランプ(縁の線も含む)を消して、4番の姿のランプを置く。消したのに新しいランプで
    覆われなかった点は、下の半分(160行目から。後ろに裾がある所)だけローブの色で埋め、上は透明にする"""
    # 持ち手の輪の行はランプの色の点が無いことがあるので、ランプ全体の囲みの中で消す
    rows = lamp_rows(a, 136, 176, 40)
    bx0 = min(v[0] for v in rows.values()) - 1
    bx1 = max(v[1] for v in rows.values()) + 1
    gone = []
    for y in range(min(rows) - 6, max(rows) + 1):
        for x in range(max(0, bx0), bx1 + 1):
            n = nm(a, x, y)
            if n in LAMP or n in EDGE or (n is not None and y < min(rows) and x < bx1 - 2):
                put(a, x, y, None)
                gone.append((x, y))
    for (x, y), n in lamp_px.items():
        put(a, x, y, n)
    for x, y in gone:
        if (x, y) not in lamp_px and y >= 160:
            put(a, x, y, 'robe')
    return a


frames = [unify_lamp(f.copy()) for f in fr[:3]] + [fr[3].copy()]

# 寝息: 横38より右の、縦145より上を1ドット下げる(継ぎ目は本の下の無地のローブ。ランプは動かさない)
SEAM, SX = 145, 38


def breath(a):
    b = a.copy()
    b[1:SEAM + 1, SX:] = a[0:SEAM, SX:]
    return b


def lamp_glow(a, mode):
    b = a.copy()
    for (x, y), n in lamp_px.items():
        if mode == 'dim' and n == 'glowL':
            put(b, x, y, 'glow')
    return b


hood = [f for f in frames for _ in range(2)]
sleep = []
for i in range(32):
    f = frames[3]
    if i >= 16:
        f = breath(f)
    # 眠っている間の灯りは弱め。4秒のうち2回だけ、ふっと元の明るさに戻る
    if not (8 <= i < 12 or 24 <= i < 28):
        f = lamp_glow(f, 'dim')
    sleep.append(f)
wake = [frames[3], frames[2], frames[1], frames[0], frames[0]]

for path, seq in zip(sys.argv[2:5], (hood, sleep, wake)):
    Image.fromarray(np.concatenate(seq, axis=1), 'RGBA').save(path)
print('ランプの点', len(lamp_px), 'かぶる', len(hood), '眠る', len(sleep), '起きる', len(wake))
