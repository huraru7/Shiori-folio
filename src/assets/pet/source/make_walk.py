# 歩く(walk)の24コマ(1周12コマを2回)を作る。生成AIの4つの姿(make_pose.pyの下絵)の上半身とローブを使い、ブーツは1コマずつ置き直す
# 生成AIの姿を並べただけだと、次の3つが違和感になった(10/26):
#   1. 支える足が地面に止まって見えない。ペットは1秒40ドット(1コマ5ドット)で進むので、
#      地面に着いた足は、絵の中で1コマごとに5ドットずつ後ろへ下がらないと、滑って見える
#   2. 体の上下が無い。歩きは「着く → 沈む(いちばん低い)→ 足を通す → 伸びる(いちばん高い)→ 着く」で上下する
#   3. 振り出す足が、後ろから前へ1コマで跳ぶ。間の位置を順に通す
# 使い方: python make_walk.py <walk_raw.png(make_pose.pyの出力)> <出力のコマ並びPNG>
#   walk_raw.png の作り方(比は、背丈815画素を待機と同じ180ドットにする4.528):
#     python make_pose.py ref/walk-20261026.png 4.528 walk_raw.png 15,345,100,935 374,718,100,935 744,1103,100,935 1130,1493,100,935
import sys
import numpy as np
from PIL import Image

from make_base import PAL

FW, FH = 128, 192
FOOT = 190
STEP_PX = 5         # 1コマで進むドット数(brain.rs の WALK_SPEED 40 ÷ 1秒8コマ)
FRONT_X = 72        # 着いた前の足のかかと側の左端
HEAD_TOP = 11       # 頭のてっぺんの基準のy
BOOT = [PAL[n] for n in ('bootD', 'boot', 'bootL')]
ROBE = [PAL[n] for n in ('robeD', 'robe', 'robeL', 'ripple', 'star')]

raw = np.array(Image.open(sys.argv[1]).convert('RGBA'))
poses = [raw[:, i * FW:(i + 1) * FW].copy() for i in range(raw.shape[1] // FW)]


def color_mask(f, cols):
    m = np.zeros(f.shape[:2], bool)
    for c in cols:
        m |= (f[..., :3] == c).all(axis=2)
    return m & (f[..., 3] > 0)


def cut_boot(f, x0, x1, y0=160):
    """範囲の中のブーツ(ブーツの色と、それに接する暗いふち)を切り出して、元の絵から消す"""
    b = color_mask(f, BOOT)
    b[:y0] = False
    b[:, :x0] = False
    b[:, x1 + 1:] = False
    near = np.zeros_like(b)
    near[1:] |= b[:-1]
    near[:-1] |= b[1:]
    near[:, 1:] |= b[:, :-1]
    near[:, :-1] |= b[:, 1:]
    dark = (f[..., :3].astype(int).sum(axis=2) < 200) & (f[..., 3] > 0)
    m = b | (near & dark & ~color_mask(f, ROBE))
    m[:y0] = False
    sprite = np.zeros_like(f)
    sprite[m] = f[m]
    f[m] = 0
    return sprite


def clean(f, y0=160, min_size=6):
    """ブーツを切り出した後の消し残り(小さな点のかたまり)を消す"""
    h, w = f.shape[:2]
    seen = np.zeros((h, w), bool)
    for y in range(y0, h):
        for x in range(w):
            if f[y, x, 3] == 0 or seen[y, x]:
                continue
            stack, comp = [(y, x)], []
            seen[y, x] = True
            while stack:
                cy, cx = stack.pop()
                comp.append((cy, cx))
                for ny in (cy - 1, cy, cy + 1):
                    for nx in (cx - 1, cx, cx + 1):
                        if y0 <= ny < h and 0 <= nx < w and not seen[ny, nx] and f[ny, nx, 3] > 0:
                            seen[ny, nx] = True
                            stack.append((ny, nx))
            if len(comp) < min_size:
                for cy, cx in comp:
                    f[cy, cx] = 0
    return f


def body_of(pose, boxes):
    """生成AIの姿から、ブーツをすべて消した体(上半身とローブ)を作る"""
    f = pose.copy()
    for b in boxes:
        cut_boot(f, *b)
    return clean(f)


def shifted(f, dy):
    out = np.zeros_like(f)
    if dy >= 0:
        out[dy:] = f[:FH - dy]
    else:
        out[:dy] = f[-dy:]
    return out


def place(canvas, sprite, x, lift, front, toe_up=0, heel_up=0):
    """ブーツを、かかと側の左端がx、床からliftドット上に置く。
    toe_up: つま先をさらに上げるドット数(着く直前)。heel_up: かかとを上げるドット数(つま先で蹴る)。
    ローブの裾より手前には出さない。front=Trueなら、もう1つのブーツより手前"""
    ys, xs = np.where(sprite[..., 3] > 0)
    x0, x1, y1 = xs.min(), xs.max(), ys.max()
    span = max(x1 - x0, 1)
    out = np.zeros_like(canvas)
    for y, xx in zip(ys, xs):
        t = (xx - x0) / span   # 0がかかと、1がつま先
        Y = y + (FOOT - y1) - lift - int(round(toe_up * t + heel_up * (1 - t)))
        X = xx - x0 + x
        if 0 <= Y < FH and 0 <= X < FW:
            out[Y, X] = sprite[y, xx]
    m = out[..., 3] > 0
    if front:
        m &= (canvas[..., 3] == 0) | color_mask(canvas, BOOT) | (np.arange(FH)[:, None] > 172) & ~color_mask(canvas, ROBE)
    else:
        m &= canvas[..., 3] == 0
    canvas[m] = out[m]


# ブーツの形: 平らに着いたブーツ(姿0の前の足)と、後ろへ蹴り上げたブーツ(姿1の後ろの足)
flat = cut_boot(poses[0].copy(), 68, 100)
kick = cut_boot(poses[1].copy(), 28, 50)
kick_lift = FOOT - np.where(kick[..., 3] > 0)[0].max()

# 体: ブーツを消した、生成AIの姿。範囲(x0, x1)は walk_raw.png の点で確かめた値
contact_a = body_of(poses[0], [(30, 60), (68, 100)])
contact_b = body_of(poses[2], [(36, 66), (68, 100)])
lifted_a = body_of(poses[1], [(28, 50), (60, 90)])
lifted_b = body_of(poses[3], [(28, 50), (66, 95)])


def head_top(f):
    return np.where((f[..., 3] > 0).any(axis=1))[0].min()


# 体の頭の高さをそろえる(上下の動きは、あとで1コマずつ付ける)
contact_a, contact_b, lifted_a, lifted_b = [shifted(b, HEAD_TOP - head_top(b)) for b in (contact_a, contact_b, lifted_a, lifted_b)]

# ちらつきを抑える: 頭・本・腕・ランプは1つの姿(REF)に固定し、裾と足だけを姿ごとの絵にする。
# 体の輪郭は、どの姿もy=110あたりまでほぼ同じ(10/26に行ごとの左右の端で確かめた)。
# それより下は、ランプのまわりの列(LAMP_COLS)だけをREFにし、裾の広がりは姿ごとの輪郭のまま残す
REF = lifted_b
UPPER = 110
LAMP_ROWS = (110, 148)
LAMP_COLS = (50, 85)


def unify(body):
    out = body.copy()
    out[:UPPER] = REF[:UPPER]
    y0, y1 = LAMP_ROWS
    x0, x1 = LAMP_COLS
    both = (body[y0:y1, x0:x1, 3] > 0) & (REF[y0:y1, x0:x1, 3] > 0)
    out[y0:y1, x0:x1][both] = REF[y0:y1, x0:x1][both]
    return out


contact_a, contact_b, lifted_a, lifted_b = [unify(b) for b in (contact_a, contact_b, lifted_a, lifted_b)]

# まばたき: 横顔の目は、目の色の点が数個だけ。閉じ目は、目の点を肌にして、まぶたの線を1本引く(REFの点で決めた座標)
EYE_PTS = [(72, 31), (73, 32), (73, 33)]
LID = [(72, 32), (73, 32), (74, 32)]
# 灯りの揺れ: ランプのガラスの中の灯りの色だけを入れ替える(make_idle.pyと同じ)
GLASS = (LAMP_COLS[0], LAMP_COLS[1], 118, 146)


def set_px(a, x, y, rgb):
    a[y, x, :3] = rgb
    a[y, x, 3] = 255


def blink(a, dy):
    for x, y in EYE_PTS:
        set_px(a, x, y + dy, PAL['skin'])
    for x, y in LID:
        set_px(a, x, y + dy, PAL['line'])


def lamp(a, dy, mode):
    x0, x1, y0, y1 = GLASS
    reg = a[y0 + dy:y1 + dy + 1, x0:x1 + 1]
    glow = color_mask(reg, [PAL['glow']])
    glow_l = color_mask(reg, [PAL['glowL']])
    if mode == 'd':
        reg[glow_l, :3] = PAL['glow']
    elif mode == 'b':
        near = np.zeros_like(glow_l)
        near[1:] |= glow_l[:-1]
        near[:-1] |= glow_l[1:]
        near[:, 1:] |= glow_l[:, :-1]
        near[:, :-1] |= glow_l[:, 1:]
        reg[glow & near, :3] = PAL['glowL']


# 1歩6コマ。体(どの姿か)・上下(+は下がる)・振り出す足(位置, 浮かせ, つま先上げ, かかと上げ, 蹴り上げの形か)
#   0 着く: 後ろの足はつま先だけ着いて、かかとが上がる
#   1 沈む: いちばん低い。後ろの足を蹴り上げる
#   2・3 足を通す: 振り出す足が、支える足の横を通る
#   4 伸びる: いちばん高い
#   5 着く直前: つま先を上げて、かかとから着く
STEP = [
    ('contact', 0, (FRONT_X - 6 * STEP_PX, 0, 0, 3, False)),
    ('contact', 1, (None, 0, 0, 0, True)),
    ('lifted', 0, (48, 5, 0, 0, False)),
    ('lifted', -1, (58, 6, 0, 0, False)),
    ('lifted', -1, (67, 4, 2, 0, False)),
    ('contact', 0, (73, 1, 3, 0, False)),
]

frames = []
for side in (0, 1):
    bodies = {'contact': (contact_a, contact_b)[side], 'lifted': (lifted_a, lifted_b)[side]}
    swing_front = side == 0     # 左右の足で、手前・奥を入れ替える
    for k, (kind, bob, (sx, lift, toe_up, heel_up, is_kick)) in enumerate(STEP):
        body = bodies[kind]
        canvas = shifted(body, bob)
        support_x = FRONT_X - k * STEP_PX
        # 奥の足を先に、手前の足を後に置く
        order = [('support', not swing_front), ('swing', swing_front)]
        order.sort(key=lambda o: o[1])
        for which, front in order:
            if which == 'support':
                place(canvas, flat, support_x, 0, front)
            elif is_kick:
                place(canvas, kick, 36, kick_lift, front)
            else:
                place(canvas, flat, sx, lift, front, toe_up, heel_up)
        frames.append(canvas)

# 1周12コマ(1.5秒)を2回くり返す24コマ(3秒)にして、まばたきを1回、灯りを不規則に揺らす
BOBS = [b for _ in (0, 1) for _, b, _ in STEP]
EYE_SEQ = {19, 20}
LAMP_SEQ = 'nnbnnndnnbnnnnbdnnnbnndn'
out = []
for i in range(24):
    f = frames[i % 12].copy()
    if i in EYE_SEQ:
        blink(f, BOBS[i % 12])
    if LAMP_SEQ[i] != 'n':
        lamp(f, BOBS[i % 12], LAMP_SEQ[i])
    out.append(f)

Image.fromarray(np.concatenate(out, axis=1)).save(sys.argv[2])
print('24コマ', sys.argv[2])
