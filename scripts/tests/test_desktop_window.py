"""Pure evidence/ownership checks; no display server or encryption executes."""
import importlib.util
from pathlib import Path
import struct
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("window_probe", Path(__file__).parents[1] / "test-desktop-window.py")
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


def image(uniform=False):
    header = [0] * 25
    header[0], header[1], header[4], header[5], header[11], header[12] = 100, 7, 320, 320, 32, 1280
    pixels = b"\0" * (320 * 320 * 4) if uniform else b"".join(struct.pack("<I", x + y * 320) for y in range(320) for x in range(320))
    return struct.pack(">25I", *header) + pixels


class WindowEvidence(unittest.TestCase):
    def test_valid_render_has_bounded_geometry_and_varied_pixels(self):
        metrics = probe.frame_metrics(image())
        self.assertEqual((320, 320), (metrics["pixel_width"], metrics["pixel_height"]))
        self.assertGreater(metrics["sampled_pixel_colors"], 32)

    def test_truncated_or_uniform_frames_do_not_qualify(self):
        for data in (b"short", image()[:-5], image(uniform=True)):
            with self.assertRaises(ValueError):
                probe.frame_metrics(data)

    def test_existing_output_is_never_overwritten(self):
        with tempfile.TemporaryDirectory(prefix="lurker-window-unit-") as root:
            output = probe.private_output(Path(root) / "new")
            self.assertEqual(0o700, output.stat().st_mode & 0o777)
            with self.assertRaises(FileExistsError):
                probe.private_output(output)

    def test_output_outside_temporary_directory_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "temporary directory"):
            probe.private_output(Path.home() / "not-a-lurker-test-output")


if __name__ == "__main__":
    unittest.main()
