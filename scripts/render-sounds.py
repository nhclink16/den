#!/usr/bin/env python3
"""Render Den's original two sine envelopes and a small matching notification set."""
import math
from pathlib import Path
import struct
import wave

root = Path(__file__).resolve().parents[1] / 'apps/web/public/sounds'
notes = {'message':[660], 'mention':[660,880], 'dm':[554,660], 'call_join':[440,554], 'call_leave':[554,440], 'someone_joined':[440], 'someone_left':[330], 'screen_share_started':[554,740], 'terminal_bell':[880], 'upload_complete':[660,880,1108], 'error':[294,262]}
for name, frequencies in notes.items():
    data = []
    for frequency in frequencies:
        for i in range(3840):
            t = i / 48000
            gain = .045 * (t / .008 if t < .008 else (0.08-t)/.072)
            data.append(round(32767 * gain * math.sin(2*math.pi*frequency*t)))
    with wave.open(str(root / f'{name}.wav'), 'wb') as out:
        out.setparams((1,2,48000,len(data),'NONE','not compressed'))
        out.writeframes(struct.pack('<'+'h'*len(data),*data))
