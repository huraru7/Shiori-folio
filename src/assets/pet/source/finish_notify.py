# ランプを掲げて知らせる(notify)の下絵(notify_raw.png)を仕上げる。使うのは2番の姿(胸の高さまで上げた所)だけ
# - 1番の姿は待機の絵を使う。3番(上げ切った姿)は、手が一色で不自然なので使わない(ふらる)
# - 顔のちらつきを防ぐため、2番の頭(縦11〜52)は待機の頭に差し替える。左の上げた手のあたりは2番のまま
# - ランプのガラスの中で、白い灯りが背景とみなされた穴は、明るい灯りの色で埋める
# - まばたき(待機と同じ目の位置)と、待機より強めの灯りの揺れを足す
# - 上げる・下げるが1コマで切り替わって速かったので(ふらる)、間の姿を2つ作る。2番の、上げた腕とランプ(縦47〜84の、
#   横44までの点と、横50までのランプ・手の色の点)だけを26・20ドット下げ、空いた所は待機の肩とマントの点(横35から)にする
# 使い方(このフォルダで):
#   python make_pose.py ref/notify-20261025.png 5.383 notify_raw.png 80,470,10,1000 540,940,10,1000 1020,1460,10,1000
#   python finish_notify.py notify_raw.png idle_base.png ../notify.png
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
idle = np.array(Image.open(sys.argv[2]).convert('RGBA'))
lift = raw[:, 128:256].copy()


def nm(a, x, y):
    if not (0 <= x < 128 and 0 <= y < 192) or a[y, x, 3] == 0:
        return None
    return INV.get(tuple(int(v) for v in a[y, x, :3]), '?')


def put(a, x, y, n):
    a[y, x, :3] = P[n]
    a[y, x, 3] = 255


# 頭を待機の頭に差し替える。2番の頭は待機より1ドット細いだけで、同じ位置にある。
# 縦44行目からは、上げた手が横44より左に来るので、そこは2番のままにする
for y in range(11, 53):
    for x in range(128):
        if y >= 44 and x <= 44:
            continue
        lift[y, x] = idle[y, x]

# ランプのガラスの中(縦70〜80・横30〜45)の、まわりを点で囲まれた透明の穴を埋める
for y in range(70, 81):
    for x in range(30, 46):
        if lift[y, x, 3] == 0:
            row = [lift[y, k, 3] for k in range(30, 46)]
            left = any(lift[y, k, 3] for k in range(30, x))
            right = any(lift[y, k, 3] for k in range(x + 1, 46))
            up = any(lift[k, x, 3] for k in range(68, y))
            down = any(lift[k, x, 3] for k in range(y + 1, 83))
            if left and right and up and down:
                put(lift, x, y, 'glowL')

# 間の姿: 上げた腕とランプを、d ドット下げる
LAMPC = {'brass', 'brassD', 'brassL', 'glow', 'glowL', 'gold', 'boot', 'bootL', 'bootD', 'skin', 'skinD', 'faceLine', 'mouth', 'page', 'star'}


def lowered(d):
    m = lift.copy()
    arm = []
    for y in range(47, 85):
        for x in range(0, 51):
            if lift[y, x, 3] and (x <= 44 or nm(lift, x, y) in LAMPC):
                arm.append((x, y))
    for x, y in arm:
        m[y, x] = idle[y, x] if x >= 35 else 0
    for x, y in arm:
        m[y + d, x] = lift[y, x]
    return m


mid_low = lowered(26)
mid_high = lowered(20)

# まばたき(待機と同じ。目は縦33〜37、左右それぞれ横54〜59・68〜73)
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
        # まぶたの線。目尻(外側)だけ1段下げて、たれ目の形を残す
        if side == 'left':
            for x in range(x0 + 1, x1 + 1):
                put(b, x, 35, 'line')
            put(b, x0, 36, 'line')
        else:
            for x in range(x0, x1):
                put(b, x, 35, 'line')
            put(b, x1, 36, 'line')
    return b


def lamp(a, mode):
    """灯りの揺れ: ガラスの中(縦68〜82・横26〜47)の橙を、明るい色か暗い色に入れ替える"""
    b = a.copy()
    for y in range(68, 83):
        for x in range(26, 48):
            n = nm(a, x, y)
            if mode == 'dim' and n == 'glowL':
                put(b, x, y, 'glow')
            if mode == 'bright' and n == 'glow':
                near = [nm(a, x + dx, y + dy) for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))]
                if any(m in ('glowL', 'gold') for m in near):
                    put(b, x, y, 'glowL')
    return b


# 並び: 1秒8コマで40コマ(知らせの5秒)。繰り返さない
# 待機1コマ → 上げる4コマ(間の姿を2コマずつ) → 掲げる29コマ(灯りは待機より明るい時間を長く、まばたき1回)
# → 下げる4コマ → 待機2コマ
LAMP_SEQ = 'bbnbbnbbbdbnbbnbbbnbbdbnbbbnb'
seq = [idle, mid_low, mid_low, mid_high, mid_high]
for i in range(29):
    f = lift
    if i in (17, 19):
        f = eyes_half(f)
    elif i == 18:
        f = eyes_closed(f)
    m = LAMP_SEQ[i]
    if m != 'n':
        f = lamp(f, 'bright' if m == 'b' else 'dim')
    seq.append(f)
seq += [mid_high, mid_high, mid_low, mid_low, idle, idle]

Image.fromarray(np.concatenate(seq, axis=1), 'RGBA').save(sys.argv[3])
print('知らせ', len(seq), 'コマ')
