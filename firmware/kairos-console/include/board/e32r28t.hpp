#pragma once

#include <Arduino.h>

namespace kairos::board {

constexpr std::uint8_t kDisplayBacklight = 21;
constexpr std::uint8_t kTouchClock = 25;
constexpr std::uint8_t kTouchMosi = 32;
constexpr std::uint8_t kTouchMiso = 39;
constexpr std::uint8_t kTouchChipSelect = 33;
constexpr std::uint8_t kTouchInterrupt = 36;

constexpr std::uint8_t kLedRed = 22;
constexpr std::uint8_t kLedGreen = 16;
constexpr std::uint8_t kLedBlue = 17;
constexpr std::uint8_t kAudioEnable = 4;
constexpr std::uint8_t kSdChipSelect = 5;
constexpr std::uint8_t kBootButton = 0;

constexpr std::uint8_t kLandscapeRotation = 1;
constexpr std::uint16_t kScreenWidth = 320;
constexpr std::uint16_t kScreenHeight = 240;

// Measured on this panel: left/right move raw_y, top/bottom move raw_x.
constexpr std::uint16_t kTouchXMinimum = 290;
constexpr std::uint16_t kTouchXMaximum = 3715;
constexpr std::uint16_t kTouchYMinimum = 460;
constexpr std::uint16_t kTouchYMaximum = 2980;
constexpr bool kTouchAxisSwap = true;

inline void prepare_safe_outputs() {
  pinMode(kLedRed, OUTPUT);
  pinMode(kLedGreen, OUTPUT);
  pinMode(kLedBlue, OUTPUT);
  digitalWrite(kLedRed, HIGH);
  digitalWrite(kLedGreen, HIGH);
  digitalWrite(kLedBlue, HIGH);

  pinMode(kAudioEnable, OUTPUT);
  digitalWrite(kAudioEnable, HIGH);
  pinMode(kSdChipSelect, OUTPUT);
  digitalWrite(kSdChipSelect, HIGH);

  pinMode(kDisplayBacklight, OUTPUT);
  digitalWrite(kDisplayBacklight, LOW);
  pinMode(kBootButton, INPUT_PULLUP);
  pinMode(kTouchInterrupt, INPUT);
}

}  // namespace kairos::board
