import importlib.util
from pathlib import Path
import unittest


SPEC = importlib.util.spec_from_file_location(
    "desktop_wayland", Path(__file__).parents[1] / "test-desktop-wayland.py"
)
wayland = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(wayland)

TRACE = '''[100.0]  -> xdg_wm_base@9.get_xdg_surface(new id xdg_surface@21, wl_surface@20)
[101.0]  -> xdg_surface@21.get_toplevel(new id xdg_toplevel@22)
[102.0]  -> xdg_toplevel@22.set_title("Lurker")
[103.0]  -> wl_surface@20.commit()
[104.0] xdg_surface@21.configure(17)
[105.0]  -> xdg_surface@21.ack_configure(17)
[106.0]  -> wl_shm_pool@24.create_buffer(new id wl_buffer@25, 0, 1280, 860, 5120, 0)
[107.0]  -> wl_surface@20.attach(wl_buffer@25, 0, 0)
[108.0]  -> wl_surface@20.commit()
'''


class WaylandTraceTests(unittest.TestCase):
    def test_configured_app_window_committed_shm_buffer(self):
        self.assertEqual(wayland.rendered_window(TRACE), {
            "surface_id": 20, "buffer_width": 1280, "buffer_height": 860, "buffer_stride": 5120,
        })

    def test_rejects_unrelated_or_unconfigured_surfaces(self):
        unrelated = TRACE.replace("wl_surface@20.attach", "wl_surface@33.attach")
        wrong_serial = TRACE.replace("ack_configure(17)", "ack_configure(18)")
        for trace in (unrelated, wrong_serial):
            with self.subTest(trace=trace):
                self.assertIsNone(wayland.rendered_window(trace))

    def test_rejects_commits_without_a_window_buffer(self):
        for trace in (TRACE.split("[106.0]")[0], TRACE.replace("1280, 860, 5120", "32, 32, 128")):
            with self.subTest(trace=trace):
                self.assertIsNone(wayland.rendered_window(trace))
