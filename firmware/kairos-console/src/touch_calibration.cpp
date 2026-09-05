#include <Arduino.h>
#include <TFT_Touch.h>

#include "board/e32r28t.hpp"

namespace {

TFT_Touch touch(kairos::board::kTouchChipSelect,
                kairos::board::kTouchClock,
                kairos::board::kTouchMosi,
                kairos::board::kTouchMiso);

std::uint32_t last_sample_ms = 0;

}  // namespace

void setup() {
  Serial.begin(115200);
  kairos::board::prepare_safe_outputs();
  touch.setResolution(kairos::board::kScreenWidth,
                      kairos::board::kScreenHeight);
  touch.setCal(0, 4095, 0, 4095, kairos::board::kScreenWidth,
               kairos::board::kScreenHeight,
               kairos::board::kTouchAxisSwap);
  Serial.println(
      "KAIROS_TOUCH press and hold each screen edge; record stable raw X/Y");
}

void loop() {
  const std::uint32_t now_ms = millis();
  if (digitalRead(kairos::board::kTouchInterrupt) == LOW && touch.Pressed() &&
      now_ms - last_sample_ms >= 100U) {
    last_sample_ms = now_ms;
    Serial.printf("KAIROS_TOUCH raw_x=%u raw_y=%u\n", touch.RawX(),
                  touch.RawY());
  }
  delay(5);
}
