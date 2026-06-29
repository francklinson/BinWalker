from PIL import Image
import os

# Create a simple 32x32 icon with a gradient
img = Image.new('RGBA', (32, 32), (0, 0, 0, 0))
pixels = img.load()

for y in range(32):
    for x in range(32):
        # Simple gradient pattern
        r = int(255 * x / 31)
        g = int(255 * y / 31)
        b = 128
        a = 255
        pixels[x, y] = (r, g, b, a)

# Save as ICO
icon_path = os.path.join('src-tauri', 'icons', 'icon.ico')
os.makedirs(os.path.dirname(icon_path), exist_ok=True)
img.save(icon_path, format='ICO', sizes=[(32, 32)])
print(f"Icon created: {icon_path}")
