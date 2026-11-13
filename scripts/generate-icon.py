"""Generate Hushlink's original app icon. Requires Pillow only for regeneration."""
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
SCALE = 4
image = Image.new("RGBA", (256 * SCALE, 256 * SCALE), (0, 0, 0, 0))
draw = ImageDraw.Draw(image)

def box(coords, radius, fill, outline=None, width=1):
    draw.rounded_rectangle(tuple(int(v * SCALE) for v in coords), radius * SCALE,
                           fill=fill, outline=outline, width=width * SCALE)

box((5, 5, 251, 251), 56, "#102A30")
box((38, 49, 172, 151), 28, "#71DAC3")
draw.polygon([(57*SCALE, 132*SCALE), (57*SCALE, 177*SCALE), (105*SCALE, 143*SCALE)], fill="#71DAC3")
box((83, 102, 219, 203), 28, "#102A30", "#B1F4E1", 12)
draw.line([(191*SCALE, 183*SCALE), (199*SCALE, 221*SCALE), (159*SCALE, 201*SCALE)], fill="#B1F4E1", width=12*SCALE, joint="curve")
for cx in (119, 151, 183):
    draw.ellipse(((cx-6)*SCALE, 147*SCALE, (cx+6)*SCALE, 159*SCALE), fill="#B1F4E1")
image = image.resize((256, 256), Image.Resampling.LANCZOS)
image.save(ROOT / "assets/hushlink.png")
image.save(ROOT / "assets/hushlink.ico", sizes=[(16,16), (24,24), (32,32), (48,48), (64,64), (128,128), (256,256)])
