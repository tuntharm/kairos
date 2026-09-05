#pragma once

#include <Arduino.h>
#include <ArduinoJson.h>
#include <TFT_eSPI.h>

namespace kairos {

struct TouchPoint {
  std::uint16_t x;
  std::uint16_t y;
};

struct AppCommand {
  String message_id;
  String action;
  JsonObjectConst payload;
};

enum class AppCommandError : std::uint8_t {
  None,
  UnsupportedAction,
  InvalidPayload,
  Busy,
};

struct AppCommandDecision {
  AppCommandError error{AppCommandError::None};
  const char* message{"accepted"};

  bool accepted() const { return error == AppCommandError::None; }
};

struct AppCommandResult {
  String message_id;
  bool success{false};
  bool confirmation_declined{false};
  String message;
  std::uint64_t revision{0};
};

class AppModule {
 public:
  virtual ~AppModule() = default;
  virtual const char* id() const = 0;
  virtual const char* title() const = 0;
  virtual const char* subtitle() const = 0;
  virtual std::uint64_t revision() const = 0;
  virtual std::uint32_t character_xp() const { return 0; }
  virtual void enter() = 0;
  virtual void render(TFT_eSPI& display, bool full_redraw) = 0;
  virtual bool on_touch(const TouchPoint& point, std::uint32_t now_ms) = 0;
  virtual bool on_tick(std::uint32_t now_ms) = 0;
  virtual void save_state() = 0;
  virtual AppCommandDecision apply_command(const AppCommand& command,
                                           std::uint32_t now_ms) = 0;
  virtual bool take_command_result(AppCommandResult& result) = 0;
  virtual void cancel_pending_command() = 0;
};

}  // namespace kairos
