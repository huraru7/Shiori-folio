# 三面図の正面の姿から、パレットに寄せた下絵を作る
import sys
from PIL import Image
import numpy as np

SRC = r'E:\portable\library\20-areas\shiori\files\shiori-character-sheet-20261023.webp'

PAL = {
    'line':   (26, 21, 48),
    'robeD':  (24, 30, 72),
    'robe':   (36, 48, 110),
    'robeL':  (52, 68, 140),
    'ripple': (120, 176, 232),
    'star':   (240, 244, 255),
    'hairD':  (128, 142, 196),
    'hair':   (178, 192, 230),
    'hairL':  (222, 230, 247),
    'skin':   (248, 222, 206),
    'skinD':  (226, 180, 166),
    'eye':    (88, 196, 164),
    'eyeD':   (36, 104, 92),
    'brassD': (110, 76, 30),
    'brass':  (176, 132, 58),
    'brassL': (226, 184, 96),
    'glow':   (255, 168, 64),
    'glowL':  (255, 230, 150),
    'bookD':  (24, 54, 38),
    'book':   (44, 88, 58),
    'gold':   (226, 192, 92),
    'bootD':  (48, 32, 26),
    'boot':   (86, 60, 44),
    'bootL':  (122, 88, 64),
    'mouth':  (196, 112, 112),
    'silver': (200, 204, 216),
}

if __name__ == '__main__':
    OUT = sys.argv[1]
    im = Image.open(SRC).convert('RGB')
    crop = im.crop((74, 22, 442, 1006))
    H = 180
    W = round(crop.width * H / crop.height)
    a = np.array(crop).astype(float)
    # 白地を背景とみなす
    bg = ((a.min(axis=2) > 235)).astype(np.uint8) * 255
    mask_small = np.array(Image.fromarray(bg).resize((W, H), Image.BOX)) < 110
    col = np.array(crop.resize((W, H), Image.BOX)).astype(float)

    names = list(PAL)
    pv = np.array([PAL[n] for n in names], float)
    # 明るさの差を少し重く見る近い色
    def nearest(c):
        d = ((pv - c) ** 2 * np.array([0.35, 0.45, 0.2])).sum(axis=1)
        return int(d.argmin())

    idx = np.full((H, W), -1, int)
    for y in range(H):
        for x in range(W):
            if mask_small[y, x]:
                idx[y, x] = nearest(col[y, x])

    canvas = np.full((192, 128), -1, int)
    ox, oy = 28, 191 - H
    canvas[oy:oy + H, ox:ox + W] = idx
    np.save(OUT, canvas)
    import json
    json.dump(names, open(OUT + '.names.json', 'w'))
