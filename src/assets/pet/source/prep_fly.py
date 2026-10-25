# 飛ぶ/落ちるの元の絵(ref/fly-20261025.png)は、隣どうしの姿が同じ列に重なっている
# (1番の裾と2番のランプ、3番のマントと4番のランプ)。make_pose.py の四角い切り出しに隣の姿が入らないよう、
# 姿ごとに、隣の姿の欠片を白にした元の絵を作る。元の絵の点そのものは変えない
# 使い方(このフォルダで): python prep_fly.py ref/fly-20261025.png fly_a.png fly_b.png
#   fly_a.png は1番・3番用、fly_b.png は2番・4番用
import sys
from PIL import Image

src = Image.open(sys.argv[1]).convert('RGB')


def whiten(img, boxes):
    out = img.copy()
    for x0, y0, x1, y1 in boxes:
        out.paste((255, 255, 255), (x0, y0, x1, y1))
    return out


# 1番・3番用: 2番のランプ(横370〜399・縦580より上)と、4番のランプ(横1129〜1139・縦560〜650)を白にする
whiten(src, [(370, 0, 400, 580), (1129, 560, 1140, 650)]).save(sys.argv[2])
# 2番・4番用: 1番の裾(横370〜391・縦580〜680)と、3番のマント(横1120〜1129・縦300〜485)を白にする
whiten(src, [(370, 580, 392, 680), (1120, 300, 1130, 485)]).save(sys.argv[3])
