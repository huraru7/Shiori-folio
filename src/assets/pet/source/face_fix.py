# 下絵(base.npy)のうち、目の枠の中の色だけを選び直し、鼻と口を足す。ほかの点は1つも変えない
# 使い方: python make_base.py base.npy の後に
#   python face_fix.py idle.png 53,35,line 54,40,skinD 61,47,skinD 54,41,faceLine 67,46,faceLine 63,40,skinD 63,43,mouth 64,43,mouth
# (front.png は三面図の正面の姿を切り出したもの。make_base.py の crop と同じ範囲)
import sys, json
import numpy as np
from PIL import Image
from make_base import PAL

PAL = dict(PAL)
PAL['eyeL'] = (180, 232, 212)
PAL['faceLine'] = (118, 96, 96)
c = np.load('base.npy')
names = json.load(open('base.npy.names.json'))
names += [n for n in PAL if n not in names]
N = {n: i for i, n in enumerate(names)}
before = c.copy()

crop = Image.open('front.png').convert('RGB')
col = np.array(crop.resize((67, 180), Image.BOX)).astype(float)
OX, OY = 28, 11

# 目の枠では、目に使う色だけから一番近い色を選ぶ(透明にはしない)
EYE_PAL = ['line', 'eyeD', 'eye', 'eyeL', 'skin', 'skinD', 'star']
pv = np.array([PAL[n] for n in EYE_PAL], float)
for x0, x1 in [(54, 60), (67, 73)]:
    for y in range(33, 38):
        for x in range(x0, x1 + 1):
            src = col[y - OY, x - OX]
            k = int(((pv - src) ** 2).sum(axis=1).argmin())
            c[y, x] = N[EYE_PAL[k]]

for x, y, n in [tuple(v.split(',')[:2]) + (v.split(',')[2],) for v in sys.argv[2:]]:
    c[int(y), int(x)] = N[n]

np.save('face.npy', c)
ch = np.argwhere(c != before)
print('変えた点の数', len(ch))
H, W = c.shape
out = np.zeros((H, W, 4), np.uint8)
for y in range(H):
    for x in range(W):
        if c[y, x] >= 0:
            out[y, x, :3] = PAL[names[c[y, x]]]
            out[y, x, 3] = 255
Image.fromarray(out, 'RGBA').save(sys.argv[1])
