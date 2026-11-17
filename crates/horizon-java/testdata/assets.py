"""Original checkerboard art and vanilla-format models/loot for both probe jars."""
import json
from pathlib import Path
import struct
import sys
import zlib

def chunk(kind, payload):
    return struct.pack('>I', len(payload)) + kind + payload + struct.pack('>I', zlib.crc32(kind + payload))

pixels = b''.join(b'\0' + b''.join(bytes((190 if (x//4 + y//4) % 2 else 245, 25, 65, 255)) for x in range(16)) for y in range(16))
png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 16, 16, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(pixels)) + chunk(b'IEND', b'')
for mod in ['fabricprobe', 'neoprobe']:
    root = Path(sys.argv[1]) / mod
    def write(path, value):
        file = root / path
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_bytes(value if isinstance(value, bytes) else json.dumps(value).encode())
    for kind, name in [('block', 'ruby_block'), ('item', 'ruby')]:
        write(f'assets/{mod}/textures/{kind}/{name}.png', png)
        write(f'assets/{mod}/items/{name}.json', {'model': {'type': 'minecraft:model', 'model': f'{mod}:{kind}/{name}'}})
    write(f'assets/{mod}/blockstates/ruby_block.json', {'variants': {'': {'model': f'{mod}:block/ruby_block'}}})
    write(f'assets/{mod}/models/block/ruby_block.json', {'parent': 'minecraft:block/cube_all', 'textures': {'all': f'{mod}:block/ruby_block'}})
    write(f'assets/{mod}/models/item/ruby.json', {'parent': 'minecraft:item/generated', 'textures': {'layer0': f'{mod}:item/ruby'}})
    write(f'data/{mod}/loot_table/blocks/ruby_block.json', {'type': 'minecraft:block', 'pools': [{'rolls': 1, 'entries': [{'type': 'minecraft:item', 'name': f'{mod}:ruby_block'}]}]})
