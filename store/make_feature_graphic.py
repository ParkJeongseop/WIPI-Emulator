# Play Store 피처 그래픽(1024x500) 생성 — 앱 브랜드(teal 배경 + 픽셀 폴더폰 아이콘)에 맞춤.
# 재생성: python3 store/make_feature_graphic.py
from PIL import Image, ImageDraw, ImageFont

W, H = 1024, 500
BG      = (0x12, 0x8C, 0x7F)  # teal (아이콘 배경과 동일)
BG_HI   = (0x17, 0xA3, 0x94)  # 대각 패턴
CREAM   = (0xFF, 0xE3, 0xC2)  # 부제 크림색
WHITE   = (0xFF, 0xFF, 0xFF)
HEART   = (0xFF, 0x4D, 0x6D)
SHADOW  = (0x0B, 0x5E, 0x55)  # 텍스트 그림자(어두운 teal)

img = Image.new("RGB", (W, H), BG)
d = ImageDraw.Draw(img)

# 은은한 대각 점 패턴 (아이콘과 통일감)
for y in range(0, H, 8):
    for x in range(0, W, 8):
        if (x // 8 + y // 8) % 2 == 0:
            d.point((x, y), fill=BG_HI)
            d.point((x + 1, y), fill=BG_HI)

# ── 아이콘: 왼쪽에 배치, 둥근 모서리
icon = Image.open("ios/WipiEmulator/Assets.xcassets/AppIcon.appiconset/icon.png").convert("RGBA")
ICON = 360
icon = icon.resize((ICON, ICON), Image.NEAREST)  # 픽셀아트라 nearest
radius = 64
mask = Image.new("L", (ICON, ICON), 0)
ImageDraw.Draw(mask).rounded_rectangle([0, 0, ICON, ICON], radius=radius, fill=255)
ix, iy = 80, (H - ICON) // 2
# 아이콘 뒤 옅은 그림자
sh = Image.new("L", (ICON + 24, ICON + 24), 0)
ImageDraw.Draw(sh).rounded_rectangle([12, 16, ICON + 12, ICON + 16], radius=radius, fill=90)
img.paste(SHADOW, (ix - 12, iy - 8), sh)
img.paste(icon, (ix, iy), mask)

# ── 텍스트: 오른쪽
tx = ix + ICON + 70
arial_bold = "/System/Library/Fonts/Supplemental/Arial Bold.ttf"
arial = "/System/Library/Fonts/Supplemental/Arial.ttf"
f_title = ImageFont.truetype(arial_bold, 92)
f_sub   = ImageFont.truetype(arial, 40)
f_tag   = ImageFont.truetype(arial, 32)

def text_shadow(xy, s, font, fill, sh_off=3):
    x, y = xy
    d.text((x + sh_off, y + sh_off), s, font=font, fill=SHADOW)
    d.text((x, y), s, font=font, fill=fill)

# 세로 중앙 정렬 블록
title1, title2 = "WIPI", "Emulator"
sub = "Korean feature-phone games"
tag = "WIPI · SKVM · J2ME"

# 대략적 높이 계산으로 수직 중앙
block_top = 150
text_shadow((tx, block_top), title1, f_title, WHITE)
text_shadow((tx, block_top + 92), title2, f_title, WHITE)
d.text((tx + 2, block_top + 92 + 108), sub, font=f_sub, fill=CREAM)
# 태그 + 하트 포인트
d.text((tx + 2, block_top + 92 + 108 + 54), tag, font=f_tag, fill=(0xBF, 0xEA, 0xE3))

img.save("store/android/feature_graphic.png")
print(f"OK {W}x{H} -> store/android/feature_graphic.png")
