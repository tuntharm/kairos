# Kairos Console

Kairos Console is the physical companion for Kairos. The ESP32 owns the
screen, touch input, saved settings, and timer.

The home screen is a cyberpunk character pane on the left and a short
scrollable app list on the right. Rank and XP come from completed Pomodoro
focus sessions. Outfit / sword unlocks are not in this build.

## Tools

```bash
cd firmware/kairos-console
pio test -e native
pio run -e e32r28t
pio run -e e32r28t -t upload --upload-port /dev/cu.usbserial-2120
```

The touch-calibration environment is `e32r28t-calibration`. Measured bounds
for this panel live in `firmware/kairos-console/include/board/e32r28t.hpp`.

Regenerate the mascot header after replacing `assets/mascot_source.png`:

```bash
python3 assets/convert_mascot.py
```
