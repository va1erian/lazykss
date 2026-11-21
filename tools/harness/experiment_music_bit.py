"""What does bit 7 of the music byte (room block offset +2) do?

    python tools/harness/experiment_music_bit.py

  question    Offset +2 of a room block is the song. Eight rooms of the
              sub-games have bit 7 set (0xB3, 0x83, 0xB5, 0xAD, 0x93, 0xA2,
              0xBA, 0xC1). Does it change what is heard?
  test        The Spring Breeze route into Green Greens (room 0, the stage
              card, then room 1), recording all sound from the stage card on,
              with the song byte of room 1 set to each of: 0x0F (the original,
              control), 0x8F, 0x0A, 0x8A, 0x10, 0x90, 0x00, 0x80. Room 0 keeps
              its own song, 0x0F, so a room 1 that "keeps the previous song"
              is told from one that starts its own.
  evidence    The emulator is deterministic, so equal sound is equal bytes. For
              each pair: byte-identical or not, and the share of samples that
              differ in the last 5 seconds (steady state, long after the
              switch). Loudness shows silence.

Result (see docs/emulator-harness.md): with bit 7 set the room does not start
its song and the previous one is stopped, not kept. Room 1 with 0x8F: the
stage card's song (0x0F) stops, a short quiet tail, then silence, where 0x0F
and 0x10 play on; 0x0A is a three-second jingle that then stops by itself.
In Revenge of Meta Knight's room 154 (0xB3) the first seconds are silent;
cleared to 0x33 the song starts at once; later the music starts anyway.
"""

import numpy as np

from emu import Emu
from session import INTO_ROOM, enter_subgame, patch, pre_state

VALUES = [0x0F, 0x8F, 0x0A, 0x8A, 0x10, 0x90, 0x00, 0x80]


def sound_of(data):
    """All sound from the stage card until 6 s after Kirby gets control."""
    state = pre_state()
    e = Emu(data)
    e.run(5)
    e.load_state(state)
    e.audio = bytearray()
    for button in INTO_ROOM:
        e.tap(button, wait=200)
    for _ in range(2):
        e.tap("A", wait=150)
    e.run(360)
    return np.frombuffer(bytes(e.audio), dtype="<i2").astype(np.int32)


sounds = {value: sound_of(patch(1, "raw", "2", f"{value:02x}")) for value in VALUES}
control = sounds[0x0F]
last = 32000 * 5 * 2  # the last five seconds of interleaved stereo samples
print("value   samples   loudness(last 5 s)   identical to 0x0F   share of samples differing from 0x0F (last 5 s)")
for value, sound in sounds.items():
    n = min(len(sound), len(control))
    same = len(sound) == len(control) and np.array_equal(sound, control)
    tail = np.abs(sound[n - last:n] - control[n - last:n])
    differing = float((tail > 0).mean())
    loud = float(np.sqrt((sound[n - last:n].astype(np.float64) ** 2).mean()))
    print(f"0x{value:02X}  {len(sound):8}  {loud:10.1f}          {str(same):5}               {differing:.3f}")
for a, b in ((0x8F, 0x0F), (0x8A, 0x0A), (0x90, 0x10), (0x80, 0x00)):
    n = min(len(sounds[a]), len(sounds[b]))
    print(f"0x{a:02X} vs 0x{b:02X}: identical = {np.array_equal(sounds[a][:n], sounds[b][:n]) and len(sounds[a]) == len(sounds[b])}")

# The timeline: loudness per second from the stage card on (the first seconds
# are the stage card, room 0, whose own song is 0x0F in every case).
print("loudness per second:")
for value in (0x0F, 0x8F, 0x0A, 0x10):
    sound = sounds[value]
    seconds = len(sound) // 64000
    row = [np.sqrt((sound[i * 64000:(i + 1) * 64000].astype(np.float64) ** 2).mean()) for i in range(seconds)]
    print(f"0x{value:02X} ", " ".join(f"{x:5.0f}" for x in row))
group = [0x8F, 0x8A, 0x90, 0x80]
print("bit-7 group identical to each other:",
      all(len(sounds[v]) == len(sounds[group[0]]) and np.array_equal(sounds[v], sounds[group[0]]) for v in group))

# Revenge of Meta Knight's first room (154) has song 0xB3. The same room with
# bit 7 cleared (0x33), and the original, from the title screen on.
print("room 154 (0xB3) against 0x33, loudness per second from the title screen:")
for label, value in (("0xB3 original", None), ("0x33 bit 7 cleared", "33"), ("0xB3 -> 0xB4", "b4")):
    data = patch(154, "raw", "2", value) if value else None
    e = enter_subgame("revenge_of_meta_knight", data, settle=700, record=True)
    sound = np.frombuffer(bytes(e.audio), dtype="<i2").astype(np.int32)
    row = [np.sqrt((sound[i * 64000:(i + 1) * 64000].astype(np.float64) ** 2).mean()) for i in range(len(sound) // 64000)]
    print(f"{label:20}", " ".join(f"{x:5.0f}" for x in row))
