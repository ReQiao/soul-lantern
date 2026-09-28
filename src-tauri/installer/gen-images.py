#!/usr/bin/env python3
"""
生成安装包用的四张图（24 位 BMP，NSIS / WiX 只认这个格式）。

    pip install pillow
    python3 src-tauri/installer/gen-images.py

素材是 src/assets/soul-lantern.png（软件背景里那盏灯笼），配色取自界面的深蓝底 + 青色光晕。
不满意可以直接用 PS 替换生成出来的 BMP，尺寸和格式对上就行：

| 文件                   | 尺寸     | 用在哪                                              |
|------------------------|----------|-----------------------------------------------------|
| nsis-sidebar.bmp       | 164×314  | setup.exe 欢迎页 / 完成页左侧竖图                   |
| nsis-header.bmp        | 150×57   | setup.exe 其余每一页左上角（右边是白底黑字的标题）  |
| wix-dialog.bmp         | 493×312  | .msi 欢迎页 / 完成页背景：左 164px 装饰，右边必须白底（上面画黑字） |
| wix-banner.bmp         | 493×58   | .msi 其余每一页顶部：左边白底放标题，右边放图        |

中文字体用文泉驿正黑（Linux 上常见）；Windows 上跑的话会退回微软雅黑。
"""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

HERE = Path(__file__).resolve().parent
LANTERN = HERE.parent.parent / "src" / "assets" / "soul-lantern.png"

NAVY_TOP = (6, 18, 44)
NAVY_BOTTOM = (3, 10, 26)
GLOW = (94, 224, 230)
TITLE = (238, 244, 255)
SUBTITLE = (147, 164, 200)

FONT_CANDIDATES = [
    "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
    "C:/Windows/Fonts/msyhbd.ttc",
    "C:/Windows/Fonts/msyh.ttc",
    "/System/Library/Fonts/PingFang.ttc",
]


def font(size: int) -> ImageFont.FreeTypeFont:
    for path in FONT_CANDIDATES:
        if Path(path).exists():
            return ImageFont.truetype(path, size)
    raise SystemExit("找不到中文字体，改一下 FONT_CANDIDATES")


def navy(w: int, h: int, horizontal: bool = False) -> Image.Image:
    """深蓝渐变底。"""
    img = Image.new("RGB", (w, h))
    px = img.load()
    span = (w if horizontal else h) - 1 or 1
    for y in range(h):
        for x in range(w):
            t = (x if horizontal else y) / span
            px[x, y] = tuple(round(a + (b - a) * t) for a, b in zip(NAVY_TOP, NAVY_BOTTOM))
    return img


def glow(img: Image.Image, cx: int, cy: int, r: int, strength: float) -> None:
    """在 (cx, cy) 叠一团青色光晕，就是界面背景灯笼后面那团光。"""
    layer = Image.new("L", img.size, 0)
    ImageDraw.Draw(layer).ellipse((cx - r, cy - r, cx + r, cy + r), fill=round(255 * strength))
    layer = layer.filter(ImageFilter.GaussianBlur(r * 0.55))
    img.paste(Image.new("RGB", img.size, GLOW), (0, 0), layer)


def lantern(height: int) -> Image.Image:
    src = Image.open(LANTERN).convert("RGBA")
    src = src.crop(src.getbbox())
    w = round(src.width * height / src.height)
    return src.resize((w, height), Image.LANCZOS)


def paste_center(img: Image.Image, sprite: Image.Image, cx: int, cy: int) -> None:
    img.paste(sprite, (cx - sprite.width // 2, cy - sprite.height // 2), sprite)


def text_center(draw: ImageDraw.ImageDraw, cx: int, y: int, text: str, f, fill) -> None:
    w = draw.textlength(text, font=f)
    draw.text((cx - w / 2, y), text, font=f, fill=fill)


def sidebar(w: int = 164, h: int = 314) -> Image.Image:
    img = navy(w, h)
    glow(img, w // 2, 118, 70, 0.55)
    paste_center(img, lantern(120), w // 2, 118)
    d = ImageDraw.Draw(img)
    text_center(d, w // 2, 214, "灵魂灯笼", font(22), TITLE)
    text_center(d, w // 2, 246, "Soul Lantern", font(12), SUBTITLE)
    text_center(d, w // 2, 264, "Minecraft 指令生成器", font(11), SUBTITLE)
    return img


def nsis_header(w: int = 150, h: int = 57) -> Image.Image:
    img = navy(w, h, horizontal=True)
    glow(img, 30, h // 2, 26, 0.5)
    paste_center(img, lantern(46), 30, h // 2)
    d = ImageDraw.Draw(img)
    d.text((58, 10), "灵魂灯笼", font=font(18), fill=TITLE)
    d.text((59, 34), "Soul Lantern", font=font(10), fill=SUBTITLE)
    return img


def wix_dialog(w: int = 493, h: int = 312) -> Image.Image:
    img = Image.new("RGB", (w, h), (255, 255, 255))
    img.paste(sidebar(164, h), (0, 0))
    return img


def wix_banner(w: int = 493, h: int = 58) -> Image.Image:
    img = Image.new("RGB", (w, h), (255, 255, 255))
    block = navy(90, h, horizontal=True)
    glow(block, 45, h // 2, 26, 0.5)
    paste_center(block, lantern(48), 45, h // 2)
    img.paste(block, (w - 90, 0))
    return img


def main() -> None:
    out = {
        "nsis-sidebar.bmp": sidebar(),
        "nsis-header.bmp": nsis_header(),
        "wix-dialog.bmp": wix_dialog(),
        "wix-banner.bmp": wix_banner(),
    }
    for name, img in out.items():
        img.convert("RGB").save(HERE / name, "BMP")
        print(f"{name}: {img.size[0]}×{img.size[1]}")


if __name__ == "__main__":
    main()
