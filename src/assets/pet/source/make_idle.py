# 待機(idle)の動きを、今の1枚の絵(顔を直した版)から作る。点は最小限しか動かさない
# 部品: 呼吸(上半身を1ドット下げる)、まばたき(半目・閉じ目)、灯りの揺れ(明るい・暗い)
# 使い方(このフォルダで): python make_idle.py idle_base.png ../idle.png
# idle_base.png は、段階4で作った1枚の絵(顔を直した版。idle.aseprite と同じもの)
import sys
import numpy as np
from PIL import Image

from make_base import PAL

PAL = dict(PAL)
PAL['eyeL'] = (180, 232, 212)
PAL['faceLine'] = (118, 96, 96)

base = np.array(Image.open(sys.argv[1]).convert('RGBA'))
H, W = base.shape[:2]


def name_at(a, x, y):
    if a[y, x, 3] == 0:
        return None
    c = tuple(int(v) for v in a[y, x, :3])
    for k, v in PAL.items():
        if v == c:
            return k
    return '?'


def put(a, x, y, n):
    a[y, x, :3] = PAL[n]
    a[y, x, 3] = 255


# 呼吸: この行より上を1ドット下げる。継ぎ目はローブの無地の所に置く
SEAM = 92


def breath(a):
    b = a.copy()
    b[1:SEAM + 1] = a[0:SEAM]
    return b


# 目の範囲(左右それぞれの目の点の横の範囲。縦は33〜37)
EYES = [(54, 59, 'out_left'), (68, 73, 'out_right')]


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
        # まぶたの線。目尻(外側)だけ1段下げて、たれ目の形を残す
        if side == 'out_left':
            for x in range(x0 + 1, x1 + 1):
                put(b, x, 35, 'line')
            put(b, x0, 36, 'line')
        else:
            for x in range(x0, x1):
                put(b, x, 35, 'line')
            put(b, x1, 36, 'line')
    return b


# ランプのガラスの中(灯りの色だけを入れ替える)
LAMP = (29, 38, 128, 137)


def lamp(a, mode):
    b = a.copy()
    x0, x1, y0, y1 = LAMP
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            n = name_at(a, x, y)
            if mode == 'dim' and n == 'glowL':
                put(b, x, y, 'glow')
            if mode == 'bright' and n == 'glow':
                near = [name_at(a, x + dx, y + dy) for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))]
                if any(m in ('glowL', 'skin', 'skinD') for m in near):
                    put(b, x, y, 'glowL')
    return b


# 並び: 1秒8コマで32コマ(4秒)。前半は吸って、後半は吐く(1ドット下がる)
N = 32
EYE_SEQ = {26: 'half', 27: 'closed', 28: 'half'}
LAMP_SEQ = 'nnbnnndnnbnnnnbdnnnbnndnnnbnnndn'

frames = []
for i in range(N):
    f = base
    if i >= 16:
        f = breath(f)
    e = EYE_SEQ.get(i)
    if e:
        # 呼吸で下げたときは、目の行も1つ下がる
        g = f if i < 16 else base
        g = eyes_half(g) if e == 'half' else eyes_closed(g)
        f = breath(g) if i >= 16 else g
    m = LAMP_SEQ[i]
    if m != 'n':
        f = lamp(f, 'bright' if m == 'b' else 'dim')
    frames.append(f)

sheet = np.concatenate(frames, axis=1)
Image.fromarray(sheet, 'RGBA').save(sys.argv[2])
print('コマ数', N, '大きさ', sheet.shape[1], 'x', sheet.shape[0])
