# 投げられて飛ぶ(fly)・落ちる(fall)の下絵(fly_raw.png。1・2番が飛ぶ、3・4番が落ちる)を仕上げる。
# 直すのは次の所だけで、顔もふくめて、ほかの点は下絵のまま(顔は姿ごとのまま。ふらる)
# - 頭の横の汗の印が浮いた点として残ったので、その点だけを消す
# - 飛ぶ・落ちるそれぞれの中で、頭のてっぺんの高さをそろえる(交互に出したときに上下に跳ねないように)
# - ランプのガラスの中で、白い灯りが背景とみなされた穴は、明るい灯りの色で埋める
# 使い方(このフォルダで。元の絵は隣の姿が重なっているので、先に prep_fly.py で姿ごとの元の絵を作る):
#   python prep_fly.py ref/fly-20261025.png fly_a.png fly_b.png
#   python make_pose.py fly_a.png 4.5 fly_p13.png 10,399,80,860 765,1139,100,840
#   python make_pose.py fly_b.png 4.5 fly_p24.png 370,765,80,850 1120,1520,100,850
#   (fly_p13.png の1コマ目・fly_p24.png の1コマ目・fly_p13.png の2コマ目・fly_p24.png の2コマ目の順に並べて fly_raw.png)
#   python finish_fly.py fly_raw.png ../fly.png ../fall.png
# 比の4.5は、顔の肌の幅が待機(20ドット)や知らせ(18〜20ドット)とそろう比。この元の絵は、ほかより一回り小さく描かれていた
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
fr = [raw[:, i * 128:(i + 1) * 128].copy() for i in range(4)]


def nm(a, x, y):
    if not (0 <= x < 128 and 0 <= y < 192) or a[y, x, 3] == 0:
        return None
    return INV.get(tuple(int(v) for v in a[y, x, :3]), '?')


def put(a, x, y, n):
    a[y, x, :3] = P[n]
    a[y, x, 3] = 255


# 汗の印の点(落ちるの髪の先の小さな点は、髪なので残す)
SWEAT = {
    0: [(41, 30), (38, 34), (39, 35), (40, 35), (38, 35)],
    1: [(40, 33), (41, 33), (38, 37), (38, 38), (39, 37)],
    2: [(92, 49), (93, 50), (93, 49)],
}
for k, pts in SWEAT.items():
    for x, y in pts:
        fr[k][y, x] = 0


def shift_up(a, d):
    b = np.zeros_like(a)
    b[:192 - d] = a[d:]
    return b


# 頭のてっぺんをそろえる(飛ぶは1番の縦21に2番を3上げ、落ちるは4番の縦30に3番を5上げる)
fr[1] = shift_up(fr[1], 3)
fr[2] = shift_up(fr[2], 5)


def fill_lamp_holes(a):
    """ランプのあたり(横45より左・縦60より下)で、上下左右を点に囲まれ、隣に灯りの色がある透明の点を埋める。
    この絵では、灯りの白い所のまわりが金や本のページの色にも寄ったので、それも灯りの色とみなす。
    持ち手の輪の中は、隣が真鍮や手の色なので透明のまま"""
    holes = []
    for y in range(60, 192):
        for x in range(0, 45):
            if a[y, x, 3]:
                continue
            enclosed = all(
                any(a[y + dy * t, x + dx * t, 3] for t in range(1, 6) if 0 <= y + dy * t < 192 and 0 <= x + dx * t < 128)
                for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)))
            near = [nm(a, x + dx, y + dy) for dx in (-1, 0, 1) for dy in (-1, 0, 1)]
            if enclosed and any(n in ('glow', 'glowL', 'gold', 'page') for n in near):
                holes.append((x, y))
    for x, y in holes:
        put(a, x, y, 'glowL')
    return len(holes)


filled = [fill_lamp_holes(f) for f in fr]

# まわりが肌や口の色に寄って、上で埋まらなかったガラスの中の穴(頭をそろえたあとの座標)
LAMP_HOLES = {1: [(28, 120), (29, 121)], 2: [(33, 137), (34, 137), (33, 138), (34, 138)], 3: [(29, 141)]}
for k, pts in LAMP_HOLES.items():
    for x, y in pts:
        if fr[k][y, x, 3] == 0:
            put(fr[k], x, y, 'glowL')

# 並び: 1秒8コマ。どちらも2つの姿を2コマずつ交互にした4コマ(0.5秒)の繰り返し
fly = [fr[0], fr[0], fr[1], fr[1]]
fall = [fr[2], fr[2], fr[3], fr[3]]
Image.fromarray(np.concatenate(fly, axis=1), 'RGBA').save(sys.argv[2])
Image.fromarray(np.concatenate(fall, axis=1), 'RGBA').save(sys.argv[3])
print('飛ぶ', len(fly), 'コマ 落ちる', len(fall), 'コマ 埋めた穴', filled)
