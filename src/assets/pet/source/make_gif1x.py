# コマ並びのPNGから、実際の大きさ(等倍)と速さ(1秒8コマ)のGIFを作る。明るい背景と暗い背景を左右に並べる
# GIFの時間は1/100秒きざみなので、120msと130msを交互にして、平均を125ms(1秒8コマ)にする
# 使い方: python make_gif1x.py <出力.gif> <コマ並び.png> [<コマ並び.png> ...](並べた順に続けて流す)
import sys
from PIL import Image
frames = []
for path in sys.argv[2:]:
    s = Image.open(path).convert('RGBA')
    for i in range(s.width // 128):
        f = s.crop((i * 128, 0, i * 128 + 128, 192))
        out = Image.new('RGB', (256, 192))
        for k, bg in enumerate([(232, 230, 239), (28, 29, 43)]):
            panel = Image.new('RGBA', (128, 192), bg + (255,))
            panel.alpha_composite(f)
            out.paste(panel.convert('RGB'), (k * 128, 0))
        frames.append(out)
durs = [120 if i % 2 == 0 else 130 for i in range(len(frames))]
frames[0].save(sys.argv[1], save_all=True, append_images=frames[1:], duration=durs, loop=0, disposal=1)
print(sys.argv[1], len(frames), 'コマ', sum(durs) / 1000, '秒')
