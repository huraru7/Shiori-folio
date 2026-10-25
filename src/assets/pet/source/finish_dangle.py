# つままれてぶら下がる(dangle)の下絵(dangle_raw.png)を仕上げる。直すのは次の所だけで、ほかの点は下絵のまま
# - 頭の位置は姿ごとのまま(上端をそろえると、頭が固定されて見えた。ふらる)
# - 頭の横の汗の印が浮いた点として残ったので、その点だけを消す
# - 顔のちらつきを防ぐため、顔のあたり(上端から50行)は3番の姿(驚いた口が分かる姿)のものを、各姿の上端に合わせて置く
# - ランプのガラスの中で、白い灯りが背景とみなされた穴は、明るい灯りの色で埋める
# 使い方(このフォルダで):
#   python make_pose.py ref/dangle-20261025.png 5.383 dangle_raw.png 70,445,0,965 535,995,0,965 1045,1485,0,965
#   python finish_dangle.py dangle_raw.png ../dangle.png
import sys
import numpy as np
from PIL import Image

from make_base import PAL

P = dict(PAL)
P['eyeL'] = (180, 232, 212)
P['faceLine'] = (118, 96, 96)
P['page'] = (236, 226, 200)
INV = {v: k for k, v in P.items()}

raw = np.array(Image.open(sys.argv[1]).convert('RGBA'))
fr = [raw[:, i * 128:(i + 1) * 128].copy() for i in range(3)]


def nm(a, x, y):
    if not (0 <= x < 128 and 0 <= y < 192) or a[y, x, 3] == 0:
        return None
    return INV.get(tuple(int(v) for v in a[y, x, :3]), '?')


def put(a, x, y, n):
    a[y, x, :3] = P[n]
    a[y, x, 3] = 255


# 汗の印の点(縮める前の座標。make_pose.py の出力のまま)
SWEAT = {0: [(41, 35), (42, 35)], 1: [(40, 48), (37, 52)], 2: [(39, 45)]}
for k, pts in SWEAT.items():
    for x, y in pts:
        fr[k][y, x] = 0

# 顔のあたりを3番に固定する。各姿の上端(フードの先。1番は縦13、2番は24、3番は15)から50行を、3番の同じ所に入れ替える
TOP = [13, 24, 15]
FACE_ROWS = 50
for k in (0, 1):
    fr[k][TOP[k]:TOP[k] + FACE_ROWS] = fr[2][TOP[2]:TOP[2] + FACE_ROWS]


def fill_lamp_holes(a):
    """ランプのあたり(横50より左・縦100より下)で、上下左右を点に囲まれ、隣に灯りの色がある透明の点を埋める。
    持ち手の輪の中は、隣が真鍮の色なので透明のまま"""
    holes = []
    for y in range(100, 192):
        for x in range(0, 50):
            if a[y, x, 3]:
                continue
            enclosed = all(
                any(a[y + dy * t, x + dx * t, 3] for t in range(1, 6) if 0 <= y + dy * t < 192 and 0 <= x + dx * t < 128)
                for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)))
            near = [nm(a, x + dx, y + dy) for dx in (-1, 0, 1) for dy in (-1, 0, 1)]
            if enclosed and any(n in ('glow', 'glowL') for n in near):
                holes.append((x, y))
    for x, y in holes:
        put(a, x, y, 'glowL')
    return len(holes)


filled = [fill_lamp_holes(f) for f in fr]

# 並び: 1秒8コマで12コマ(1.5秒)の繰り返し。ぶら下がって(0.5秒)、足をばたつかせる(2・3番を2コマずつ交互に)
seq = [fr[0]] * 4 + [fr[1], fr[1], fr[2], fr[2], fr[1], fr[1], fr[2], fr[2]]
Image.fromarray(np.concatenate(seq, axis=1), 'RGBA').save(sys.argv[2])
print('つままれる', len(seq), 'コマ 埋めた穴', filled)
