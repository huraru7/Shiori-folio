# 座って読むの下絵(read_raw.png)を仕上げる。直すのはランプの所だけで、ほかの点は下絵のまま
# - ランプは、キャラクターの右側(画面の左)の1つにそろえる。4番の姿の左のランプを、座る・読むの全部の姿で同じ位置に置く
# - 画面の右側のランプは消し、ランプの後ろから右へはみ出していた裾も、その行だけ消す
# - 顔のちらつきを防ぐため、ページをめくる姿は本と手のあたりだけを使う。まばたきと灯りの揺れを足す
# 使い方(このフォルダで): python finish_read.py read_raw.png idle_base.png ../idle.png sit.png ../read.png
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
idle_seq = np.array(Image.open(sys.argv[3]).convert('RGBA'))
fr = [raw[:, i * 128:(i + 1) * 128].copy() for i in range(raw.shape[1] // 128)]

ROBE = {'robe', 'robeD', 'robeL', 'line', 'hairD', 'hair', 'ripple', 'silver', 'hairL', 'star', 'book', 'bookD'}
LAMP = {'brassD', 'brass', 'brassL', 'glow', 'glowL', 'gold', 'bootD', 'boot', 'bootL', 'page', 'skin', 'skinD', 'star', 'mouth'}


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


# 4番の姿(frame 3)の左のランプを切り出す
lamp_px = {}
for y in range(143, 177):
    # その行の、ランプの色の点の左端から右端まで(間の透明な穴も含める)
    xs = [x for x in range(15, 35) if nm(fr[3], x, y) in LAMP | {'faceLine'}]
    if not xs:
        continue
    for x in range(xs[0], xs[-1] + 1):
        # ガラスの中(159行目から)で、白い灯りが背景とみなされた穴は、明るい灯りの色で埋める。
        # 上の持ち手の輪の中は、透明のまま
        n = nm(fr[3], x, y) or ('glowL' if y >= 159 else None)
        if n:
            lamp_px[(x, y)] = n
# 175行目より下は裾が左に出てくるので、ランプの底の行だけにする
lamp_px = {k: v for k, v in lamp_px.items() if not (k[1] >= 175 and k[0] < 20)}


def remove_lamp(a, x_from, y0, y1):
    """画面の右側のランプを消す。ランプの後ろから右へはみ出していた裾も、その行だけ消す
    (裾を埋め足すと、細い羽のような形が出たため。縁はランプの左の境のままにする)"""
    gone = set()
    for y in range(y0, y1):
        for x in range(x_from, 128):
            if nm(a, x, y) in LAMP:
                gone.add((x, y))
    # ランプの左の縁の線(faceLine)も、ランプの一部として消す
    for y in range(y0, y1):
        for x in range(x_from, 128):
            if nm(a, x, y) == 'faceLine' and any((x + d, y + e) in gone for d in (-1, 0, 1) for e in (-1, 0, 1)):
                gone.add((x, y))
    for y in range(y0, y1):
        xs = [x for x, yy in gone if yy == y]
        if xs:
            for x in range(min(xs), 128):
                put(a, x, y, None)
    # ランプの持ち手の輪など、離れて残った小さなかたまり(6点未満)を消す
    seen = set()
    for y in range(y0, y1):
        for x in range(x_from, 128):
            if a[y, x, 3] and (x, y) not in seen:
                comp, st = [], [(x, y)]
                seen.add((x, y))
                while st:
                    cx, cy = st.pop()
                    comp.append((cx, cy))
                    for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)):
                        q = (cx + dx, cy + dy)
                        if 0 <= q[0] < 128 and 0 <= q[1] < 192 and q not in seen and a[q[1], q[0], 3]:
                            seen.add(q)
                            st.append(q)
                if len(comp) < 6:
                    for q in comp:
                        put(a, q[0], q[1], None)
    return gone


def place_lamp(a):
    for (x, y), n in lamp_px.items():
        put(a, x, y, n)


def erase_left_lamp(a, x1, y0, y1, fill_robe_rows=()):
    """3番の姿の、元のランプを消す(新しいランプを同じあたりに置く前に)"""
    for y in range(y0, y1):
        for x in range(0, x1):
            if y in fill_robe_rows:
                if nm(a, x, y) in LAMP or nm(a, x, y) == 'faceLine':
                    put(a, x, y, 'robe')
            elif x < x1 - 2 or nm(a, x, y) in LAMP:
                put(a, x, y, None)


# --- 座る: 待機の絵 → 2番 → 3番 → 4番 ---
s2 = fr[2].copy()
erase_left_lamp(s2, 30, 130, 172, fill_robe_rows=range(168, 172))
place_lamp(s2)
s3 = fr[3].copy()
remove_lamp(s3, 93, 136, 185)
# 1コマずつだと座るのが速いので、1・3・2・2コマずつ見せる(8コマ・1秒)
sit = [idle] + [fr[1]] * 3 + [s2] * 2 + [s3] * 2

# --- 読む: 8番の姿を元に、6・7番でページをめくる ---
reads = {}
for k in (5, 6, 7):
    a = fr[k].copy()
    remove_lamp(a, 101, 125, 192)
    place_lamp(a)
    reads[k] = a

# ページをめくる姿(6・7番)は、本と手のあたり(縦98〜153・横28〜97)だけを使い、ほかは8番の姿のままにする。
# 姿ごとに縮めると、顔や裾の点がコマの間でちらつくため
BOOK = (28, 98, 98, 154)
for k in (5, 6):
    x0, x1, y0, y1 = BOOK
    t = reads[7].copy()
    t[y0:y1, x0:x1] = reads[k][y0:y1, x0:x1]
    reads[k] = t



def eyes_closed(a):
    """読むときの閉じ目。目(縦91〜94)を肌にして、まぶたの線を1本引く。目尻だけ1段下げる"""
    b = a.copy()
    for x0, x1, outer in ((52, 58, 52), (67, 73, 73)):
        for y in range(91, 95):
            for x in range(x0, x1 + 1):
                put(b, x, y, 'skin')
        for x in range(x0, x1 + 1):
            put(b, x, 93 if x == outer else 92, 'line')
    return b


def lamp(a, mode):
    """灯りの揺れ(待機と同じ): ガラスの中の橙を、明るい色か暗い色に入れ替える"""
    b = a.copy()
    for y in range(159, 173):
        for x in range(17, 36):
            n = nm(a, x, y)
            if mode == 'dim' and n == 'glowL':
                put(b, x, y, 'glow')
            if mode == 'bright' and n == 'glow':
                near = [nm(a, x + dx, y + dy) for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))]
                if any(m in ('glowL', 'skin', 'skinD') for m in near):
                    put(b, x, y, 'glowL')
    return b


# 並び: 1秒8コマで32コマ(4秒)。ページをめくるのは4秒に1回、まばたきも1回。灯りは待機と同じ並びで揺らす
LAMP_SEQ = 'nnbnnndnnbnnnnbdnnnbnndnnnbnnndn'
read_seq = []
for i in range(32):
    f = {24: reads[5], 25: reads[6], 26: reads[6]}.get(i, reads[7])
    if i in (10, 11):
        f = eyes_closed(f)
    if LAMP_SEQ[i] != 'n':
        f = lamp(f, 'bright' if LAMP_SEQ[i] == 'b' else 'dim')
    read_seq.append(f)

Image.fromarray(np.concatenate(sit, axis=1), 'RGBA').save(sys.argv[4])
Image.fromarray(np.concatenate(read_seq, axis=1), 'RGBA').save(sys.argv[5])
print('ランプの点', len(lamp_px), '座る', len(sit), '読む', len(read_seq))
