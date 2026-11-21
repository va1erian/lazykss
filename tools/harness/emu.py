"""A headless SNES emulator for scripts: a ctypes frontend over a libretro core.

No window, no audio. A script presses buttons, runs frames, and reads memory:

    from emu import Emu
    e = Emu()                      # boots the ROM
    e.run(600)                     # run 600 frames with nothing pressed
    e.run(8, ["Start"])            # hold Start for 8 frames
    e.shot("title.png")            # save what is on screen
    wram = e.wram()                # 128 KiB of work RAM, as bytes

The core must be a bsnes-family libretro core (SA-1 support is required). It
was developed against `bsnes_mercury_balanced_libretro.dll` from a RetroArch
install; set `KSS_LIBRETRO_CORE` to the core's path. Other cores answer
different option keys and lay their savestates out differently: see
`VARIABLES` and `oam()`.

See docs/emulator-harness.md for how to use this to answer questions about
the game.
"""

import ctypes as C
import os
import sys
from pathlib import Path

from PIL import Image

DEFAULT_ROM_NAME = "Kirby Super Star (USA).sfc"
DEFAULT_CORE = r"E:/Games/RetroArch-Win64/cores/bsnes_mercury_balanced_libretro.dll"

# libretro joypad button ids.
BUTTONS = dict(B=0, Y=1, Select=2, Start=3, Up=4, Down=5, Left=6, Right=7, A=8, X=9, L=10, R=11)

# libretro environment commands this frontend answers.
ENV_GET_CAN_DUPE = 3
ENV_GET_SYSTEM_DIRECTORY = 9
ENV_SET_PIXEL_FORMAT = 10
ENV_GET_VARIABLE = 15
ENV_SET_VARIABLES = 16
ENV_GET_VARIABLE_UPDATE = 17
ENV_GET_LOG_INTERFACE = 27
ENV_GET_SAVE_DIRECTORY = 31

# libretro memory ids.
MEMORY_SAVE_RAM = 0   # the cartridge's 8 KiB BW-RAM: SA-1 game state lives here
MEMORY_SYSTEM_RAM = 2  # WRAM, 128 KiB: 7E:0000-7F:FFFF
MEMORY_VIDEO_RAM = 3   # VRAM, 64 KiB

# Core options. bsnes-mercury dereferences the answer without checking that
# the frontend gave one, so every key it asks for must get a value: an
# unknown key is answered "disabled".
VARIABLES = {
    "bsnes_chip_hle": b"LLE",
    "bsnes_superfx_overclock": b"100%",
    "bsnes_region": b"auto",
    "bsnes_aspect_ratio": b"auto",
    "bsnes_gamma_ramp": b"disabled",
    "bsnes_crop_overscan": b"disabled",
    "bsnes_sgb_core": b"SameBoy",
}


def find_rom():
    """The ROM: `KSS_ROM`, else the default name in this directory or a parent."""
    env = os.environ.get("KSS_ROM")
    if env and Path(env).is_file():
        return Path(env)
    for folder in [Path.cwd(), *Path.cwd().parents]:
        if (folder / DEFAULT_ROM_NAME).is_file():
            return folder / DEFAULT_ROM_NAME
    sys.exit("ROM not found: set KSS_ROM or put '%s' in a parent directory" % DEFAULT_ROM_NAME)


def find_core():
    path = Path(os.environ.get("KSS_LIBRETRO_CORE", DEFAULT_CORE))
    if not path.is_file():
        sys.exit("libretro core not found at %s: set KSS_LIBRETRO_CORE" % path)
    return path


class _GameInfo(C.Structure):
    _fields_ = [("path", C.c_char_p), ("data", C.c_void_p), ("size", C.c_size_t), ("meta", C.c_char_p)]


class _Variable(C.Structure):
    _fields_ = [("key", C.c_char_p), ("value", C.c_char_p)]


_ENV = C.CFUNCTYPE(C.c_bool, C.c_uint, C.c_void_p)
_VIDEO = C.CFUNCTYPE(None, C.c_void_p, C.c_uint, C.c_uint, C.c_size_t)
_AUDIO = C.CFUNCTYPE(None, C.c_int16, C.c_int16)
_AUDIO_BATCH = C.CFUNCTYPE(C.c_size_t, C.c_void_p, C.c_size_t)
_POLL = C.CFUNCTYPE(None)
_INPUT = C.CFUNCTYPE(C.c_int16, C.c_uint, C.c_uint, C.c_uint, C.c_uint)
# The log callback is variadic in C; only its fixed arguments are declared.
_LOG = C.CFUNCTYPE(None, C.c_int, C.c_char_p)


class _LogCallback(C.Structure):
    _fields_ = [("log", _LOG)]


class Emu:
    """One running game.

    The core keeps global state: making a new `Emu` restarts the game for
    every `Emu` of the process, so use one at a time.
    """

    def __init__(self, rom=None, core=None):
        """Boots `rom`: a path, the ROM's bytes, or None for the default ROM.

        Passing bytes is how a patched ROM is run without writing it to disk.
        """
        if rom is None:
            rom = find_rom()
        self.rom = Path(rom).read_bytes() if isinstance(rom, (str, Path)) else bytes(rom)
        self.pressed = set()
        self.frame = None
        self.audio = None  # a bytearray of 16-bit stereo samples while recording
        self._work_dir = C.c_char_p(os.getcwd().encode())
        self.lib = lib = C.CDLL(str(core or find_core()))

        def environment(command, data):
            if command == ENV_SET_PIXEL_FORMAT:
                return True
            if command in (ENV_GET_SYSTEM_DIRECTORY, ENV_GET_SAVE_DIRECTORY):
                C.cast(data, C.POINTER(C.c_char_p))[0] = self._work_dir.value
                return True
            if command == ENV_GET_VARIABLE:
                variable = C.cast(data, C.POINTER(_Variable))[0]
                variable.value = VARIABLES.get(variable.key.decode(), b"disabled")
                return True
            if command == ENV_GET_VARIABLE_UPDATE:
                C.cast(data, C.POINTER(C.c_bool))[0] = False
                return True
            if command == ENV_GET_CAN_DUPE:
                C.cast(data, C.POINTER(C.c_bool))[0] = True
                return True
            if command == ENV_GET_LOG_INTERFACE:
                # Without a logger the core prints its start-up chatter on
                # the script's own output. Set KSS_EMU_LOG=1 to see it.
                if os.environ.get("KSS_EMU_LOG"):
                    return False
                C.cast(data, C.POINTER(_LogCallback))[0].log = self._discard_log
                return True
            return command == ENV_SET_VARIABLES

        def video(data, width, height, pitch):
            if data:
                self.frame = (C.string_at(data, pitch * height), width, height, pitch)

        def joypad(port, _device, _index, button):
            return 1 if port == 0 and button in self.pressed else 0

        def audio_batch(data, frames):
            if self.audio is not None:
                self.audio += C.string_at(data, frames * 4)
            return frames

        self._discard_log = _LOG(lambda level, message: None)
        # The callbacks must outlive the calls that register them.
        self._callbacks = [
            _ENV(environment),
            _VIDEO(video),
            _AUDIO(lambda left, right: None),
            _AUDIO_BATCH(audio_batch),
            _POLL(lambda: None),
            _INPUT(joypad),
        ]
        lib.retro_set_environment(self._callbacks[0])
        lib.retro_init()
        lib.retro_set_video_refresh(self._callbacks[1])
        lib.retro_set_audio_sample(self._callbacks[2])
        lib.retro_set_audio_sample_batch(self._callbacks[3])
        lib.retro_set_input_poll(self._callbacks[4])
        lib.retro_set_input_state(self._callbacks[5])

        self._rom_buffer = C.create_string_buffer(self.rom, len(self.rom))
        info = _GameInfo(b"game.sfc", C.cast(self._rom_buffer, C.c_void_p), len(self.rom), None)
        lib.retro_load_game.argtypes = [C.POINTER(_GameInfo)]
        lib.retro_load_game.restype = C.c_bool
        if not lib.retro_load_game(C.byref(info)):
            raise RuntimeError("the core refused the ROM")
        lib.retro_get_memory_data.restype = C.c_void_p
        lib.retro_get_memory_data.argtypes = [C.c_uint]
        lib.retro_get_memory_size.restype = C.c_size_t
        lib.retro_get_memory_size.argtypes = [C.c_uint]
        lib.retro_serialize_size.restype = C.c_size_t
        lib.retro_serialize.argtypes = [C.c_void_p, C.c_size_t]
        lib.retro_serialize.restype = C.c_bool
        lib.retro_unserialize.argtypes = [C.c_void_p, C.c_size_t]
        lib.retro_unserialize.restype = C.c_bool

    # ---- running ----

    def run(self, frames=1, buttons=()):
        """Runs `frames` frames (60 per second) with `buttons` held."""
        self.pressed = {BUTTONS[name] for name in buttons}
        for _ in range(frames):
            self.lib.retro_run()

    def tap(self, button, hold=8, wait=170):
        """Presses `button` for `hold` frames, then waits `wait` frames."""
        self.run(hold, [button])
        self.run(wait)

    def record_audio(self, frames, buttons=()):
        """Runs `frames` frames and returns the sound they made: a numpy-ready
        bytes object of interleaved signed 16-bit stereo samples (32 kHz).

        The emulator is deterministic, so the same song started at the same
        moment gives the same samples: equal bytes mean equal sound.
        """
        self.audio = bytearray()
        self.run(frames, buttons)
        taken, self.audio = bytes(self.audio), None
        return taken

    # ---- memory ----

    def memory(self, memory_id):
        pointer = self.lib.retro_get_memory_data(memory_id)
        size = self.lib.retro_get_memory_size(memory_id)
        return C.string_at(pointer, size) if pointer and size else b""

    def wram(self):
        """Work RAM. Index 0 is 7E:0000; index 0x10000 is 7F:0000."""
        return self.memory(MEMORY_SYSTEM_RAM)

    def vram(self):
        """Video RAM, 64 KiB. Character `n` (4bpp) is the 32 bytes at `n * 32`."""
        return self.memory(MEMORY_VIDEO_RAM)

    def bwram(self):
        """The cartridge RAM (6000-7FFF on the SA-1 side), 8 KiB."""
        return self.memory(MEMORY_SAVE_RAM)

    def poke_bwram(self, offset, value):
        """Writes one byte of cartridge RAM (see session.py, `UNLOCK`)."""
        C.memset(self.lib.retro_get_memory_data(MEMORY_SAVE_RAM) + offset, value, 1)

    def state(self):
        """The core's savestate: every register and memory, in the core's own layout."""
        size = self.lib.retro_serialize_size()
        buffer = C.create_string_buffer(size)
        if not self.lib.retro_serialize(buffer, size):
            raise RuntimeError("the core could not serialize")
        return buffer.raw

    def load_state(self, state):
        """Restores a savestate. The ROM may differ from the one it was made
        with: the state holds RAM, not ROM, so a patched ROM can resume from a
        state taken before the patched data is read."""
        buffer = C.create_string_buffer(state, len(state))
        if not self.lib.retro_unserialize(buffer, len(state)):
            raise RuntimeError("the core refused the state")

    def palette(self):
        """The 256 CGRAM colours as BGR555 words.

        Read from the game's own mirror at 7E:0500, which this game keeps in
        step with CGRAM. The core does not expose CGRAM.
        """
        wram = self.wram()
        return [wram[0x500 + i * 2] | wram[0x501 + i * 2] << 8 for i in range(256)]

    def oam(self):
        """The sprites on screen, front first: `(x, y, character, attributes, large)`.

        `character` is 9 bits (0-511); `large` is 1 for a 16x16 sprite, 0 for
        8x8. Sprites parked below the screen are left out.

        The core does not expose OAM, so it is read out of the savestate. In
        bsnes-mercury's layout the 544 bytes of OAM sit exactly 0x20000 bytes
        after the start of VRAM; `selfcheck.py` verifies that still holds.
        """
        state = self.state()
        vram = self.vram()
        start = state.find(vram[0x20:0x420])
        if start < 0:
            raise RuntimeError("VRAM not found in the savestate: another core?")
        table = state[start - 0x20 + 0x20000:][:544]
        sprites = []
        for index in range(128):
            x, y, character, attributes = table[index * 4:index * 4 + 4]
            high = (table[512 + index // 4] >> (index % 4 * 2)) & 3
            if high & 1:
                x -= 256
            if y < 224:
                sprites.append((x, y, character | (attributes & 1) << 8, attributes, high >> 1))
        return sprites

    # ---- screen ----

    def shot(self, path=None):
        """The last frame as a PIL image (256x224); saved to `path` if given."""
        data, width, height, pitch = self.frame
        if pitch >= width * 4:
            image = Image.frombuffer("RGBA", (pitch // 4, height), data, "raw", "BGRA", 0, 1).convert("RGB")
        else:
            image = Image.frombuffer("RGB", (pitch // 2, height), data, "raw", "BGR;15", 0, 1)
        image = image.crop((0, 0, width, height))
        if path:
            image.save(path)
        return image


def contact_sheet(images, path, columns=4):
    """Saves frames side by side: the quickest way to see what a run did."""
    rows = (len(images) + columns - 1) // columns
    sheet = Image.new("RGB", (256 * columns, 224 * rows))
    for index, image in enumerate(images):
        sheet.paste(image, (index % columns * 256, index // columns * 224))
    sheet.save(path)
