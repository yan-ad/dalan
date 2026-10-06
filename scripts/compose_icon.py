#!/usr/bin/env python3
"""Composite the supplied PNG alpha silhouette as #FF6AEB on opaque black.

Build-time only: standard library, no image-generation or runtime dependencies.
Accept only the 1024-square, non-interlaced 8-bit RGBA source contract.
"""
import struct
import sys
import zlib
from pathlib import Path

SIGNATURE = b"\x89PNG\r\n\x1a\n"
LOGO_RGB = (255, 106, 235)
MAX_BYTES = 8 * 1024 * 1024


def decode_rgba(data):
    if len(data) > MAX_BYTES or not data.startswith(SIGNATURE):
        raise ValueError("Invalid source PNG")
    position = 8
    payload = bytearray()
    dimensions = None
    ended = False
    while position + 12 <= len(data):
        length = struct.unpack_from(">I", data, position)[0]
        kind = data[position + 4:position + 8]
        end = position + 8 + length
        if end + 4 > len(data):
            raise ValueError("Truncated PNG chunk")
        chunk = data[position + 8:end]
        crc = struct.unpack_from(">I", data, end)[0]
        if zlib.crc32(kind + chunk) != crc:
            raise ValueError("Invalid PNG checksum")
        if kind == b"IHDR":
            if dimensions is not None or length != 13:
                raise ValueError("Invalid PNG header")
            width, height, depth, color, compression, filtering, interlace = struct.unpack(">IIBBBBB", chunk)
            if (width, height, depth, color, compression, filtering, interlace) != (1024, 1024, 8, 6, 0, 0, 0):
                raise ValueError("Source must be 1024-square non-interlaced 8-bit RGBA")
            dimensions = (width, height)
        elif kind == b"IDAT":
            payload.extend(chunk)
        elif kind == b"IEND":
            if length != 0 or end + 4 != len(data):
                raise ValueError("Invalid PNG end")
            ended = True
            break
        elif kind[:1].isupper():
            raise ValueError("Unsupported critical PNG chunk")
        position = end + 4
    if dimensions is None or not ended:
        raise ValueError("Incomplete PNG")
    width, height = dimensions
    stride = width * 4
    expected = height * (stride + 1)
    decoder = zlib.decompressobj()
    raw = decoder.decompress(payload, expected + 1)
    if len(raw) != expected or not decoder.eof or decoder.unused_data:
        raise ValueError("Invalid or oversized PNG pixels")
    pixels = bytearray()
    previous = bytearray(stride)
    for y in range(height):
        start = y * (stride + 1)
        method = raw[start]
        if method > 4:
            raise ValueError("Unsupported PNG filter")
        row = bytearray(raw[start + 1:start + 1 + stride])
        for i in range(stride):
            left = row[i - 4] if i >= 4 else 0
            above = previous[i]
            diagonal = previous[i - 4] if i >= 4 else 0
            prediction = 0
            if method == 1:
                prediction = left
            elif method == 2:
                prediction = above
            elif method == 3:
                prediction = (left + above) // 2
            elif method == 4:
                p = left + above - diagonal
                distances = (abs(p - left), abs(p - above), abs(p - diagonal))
                prediction = (left, above, diagonal)[distances.index(min(distances))]
            row[i] = (row[i] + prediction) & 255
        pixels.extend(row)
        previous = row
    return width, height, pixels


def chunk(kind, contents):
    return struct.pack(">I", len(contents)) + kind + contents + struct.pack(">I", zlib.crc32(kind + contents))


def encode_rgba(width, height, pixels):
    stride = width * 4
    rows = b"".join(b"\0" + pixels[y * stride:(y + 1) * stride] for y in range(height))
    return (SIGNATURE + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
            + chunk(b"sRGB", b"\0") + chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b""))


def compose(data):
    width, height, pixels = decode_rgba(data)
    for i in range(0, len(pixels), 4):
        alpha = pixels[i + 3]
        pixels[i:i + 4] = bytes((*((color * alpha + 127) // 255 for color in LOGO_RGB), 255))
    return encode_rgba(width, height, pixels)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit("usage: compose_icon.py SOURCE.png OUTPUT.png")
    source = Path(sys.argv[1])
    if source.stat().st_size > MAX_BYTES:
        raise SystemExit("Source PNG exceeds size limit")
    Path(sys.argv[2]).write_bytes(compose(source.read_bytes()))
